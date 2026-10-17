//! Field talk: the message text engine and YS::WINDOW_TALK, translated from
//! the PC executable.
//!
//! - Text: control-code lengths (table 0x1405c4cd0, FUN_1403ae2e0), line and
//!   page measurement (FUN_1403ae680 / FUN_1403aeaa0), drawing with the
//!   typewriter budget (FUN_1403afd90 / FUN_1403b0280 / FUN_1403aff50) and
//!   the glyph quad (FUN_1404070f0).
//! - Window: base ctor FUN_1403bb2d0 / TALK ctor FUN_1403bb700, update
//!   FUN_1403bce30, close FUN_1403bcb40 / FUN_1403bc170, draw FUN_1403bc3b0 /
//!   FUN_1403bbeb0 with the nine-slice box FUN_140193430.
//! - Talk scripts: libretto ops (FUN_1403ec260).
//!
//! Layout units are the PS2 512x416 space; windows have no X origin, so
//! they stay centred on a 16:9 canvas.
use crate::gauge::Draw;
use data::sprite_sequence::{Modifiers, Player, Quad, Sequence, color};

/// Event font (font id 3) descriptor for international text: 24x32 glyph
/// cells in a 512x512 plane (21 columns), reference height 24, line spacing
/// 2, baseline nudge +2 px (DAT_142a250c0 = 0x20 GS units).
pub const CELL: [f32; 2] = [24., 32.];
pub const COLUMNS: usize = 21;
pub const REFERENCE_HEIGHT: f32 = 24.;
pub const LINE_SPACING: i32 = 2;
pub const BASELINE: f32 = 2.;

/// WINDOW_TALK: position (0x100, 0x150), font 3, colour id 5, type 0,
/// alignment 3 (centred); tail on, kind 1, position 0.
pub const TALK_POSITION: [i32; 2] = [256, 336];
/// Box groups by window type t: open 67+3t, idle 68+3t, close 69+3t (the
/// byte string at 0x14074b358).
pub const BOX_OPEN: usize = 67;
/// Advance arrow (mode 1): appear+bounce, vanish.
pub const ARROW: usize = 65;
pub const ARROW_CLOSE: usize = 66;
/// Tail anchor factors by position & 3 (0x14074b378).
const TAIL_FACTORS: [f32; 4] = [-0.5, -0.15, 0.15, 0.5];
/// se000 sound played on a page advance or close (FUN_1401dd170(0x18)).
pub const ADVANCE_SOUND: u32 = 24;

/// Total byte length of the code starting with `b` (0x1405c4cd0); 0x18 is
/// unused and counted as one byte here.
pub fn code_length(b: u8) -> usize {
    match b {
        0x04 | 0x06 | 0x09..=0x0c | 0x0e | 0x16 | 0x19..=0x1f => 2,
        0x05 => 7,
        0x07 | 0x11 | 0x13 => 5,
        0x08 => 4,
        0x0f => 6,
        0x12 | 0x14 | 0x15 | 0x17 => 3,
        _ => 1,
    }
}

fn next(bytes: &[u8], i: usize) -> usize {
    match bytes.get(i) {
        Some(&b) if b < 0x20 => i + code_length(b),
        _ => i + 1,
    }
}

/// International glyph index (FUN_1403ae520): byte − 0x20, with 0x46 read
/// as 0x34 unless a neighbour is 0x66.
pub(crate) fn glyph_index(bytes: &[u8], i: usize) -> Option<usize> {
    let b = *bytes.get(i)?;
    if b < 0x20 {
        return None;
    }
    let idx = (b - 0x20) as usize;
    if idx == 0x46 && bytes.get(i + 1) != Some(&0x66) && (i == 0 || bytes[i - 1] != 0x66) {
        return Some(0x34);
    }
    Some(idx)
}

/// Scale state shared by measurement and drawing (codes 03, 0a, 0b, 0c).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Style {
    scale: f32,
    width_scale: f32,
    extra: i32,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            scale: 1.,
            width_scale: 1.,
            extra: 0,
        }
    }
}

impl Style {
    fn apply(&mut self, bytes: &[u8], i: usize) {
        let arg = bytes.get(i + 1).copied().unwrap_or(0);
        match bytes[i] {
            0x03 => {
                self.scale = 1.;
                self.width_scale = 1.;
            }
            0x0a => self.scale = arg as f32 / REFERENCE_HEIGHT,
            0x0b => self.width_scale = arg as f32 / 100.,
            0x0c => self.extra = arg as i32,
            _ => {}
        }
    }
}

/// Advance of a glyph or space in pixels (international: +0.5 rounding).
fn advance(width: u8, s: &Style) -> i32 {
    (width as f32 * s.scale * s.width_scale + 0.5) as i32
}

fn space(s: &Style) -> i32 {
    ((CELL[0] as i32 / 3) as f32 * s.width_scale * s.scale + 0.5) as i32
}

/// FUN_1403ae680: one line from `i`: (stop index, width, height). Stops at
/// a newline (after it), a page break or the end (at it).
fn measure_line(bytes: &[u8], mut i: usize, widths: &[u8], s: &mut Style) -> (usize, i32, i32) {
    let mut w = 0;
    let mut h = (REFERENCE_HEIGHT * s.scale + 0.5) as i32;
    let mut previous = None;
    while let Some(&b) = bytes.get(i) {
        match b {
            0x00 | 0x10 => return (i, w, h),
            0x01 => w += space(s),
            0x02 => return (next(bytes, i), w, h),
            0x03 | 0x0a | 0x0b | 0x0c => s.apply(bytes, i),
            0x15 | 0x16 => w += 0x20,
            b if b >= 0x20 => {
                let idx = glyph_index(bytes, i).unwrap_or(0);
                let mut width = widths.get(idx).copied().unwrap_or(0) as i32;
                if idx == 0x46 && previous == Some(0x46) {
                    width -= 2;
                }
                w += advance(width.max(0) as u8, s);
                h = h.max((REFERENCE_HEIGHT * s.scale) as i32);
                previous = Some(idx);
            }
            _ => {}
        }
        i = next(bytes, i);
    }
    (i, w, h)
}

/// One page from `i`: (width, height, start of the next page if any).
fn measure_page(bytes: &[u8], mut i: usize, widths: &[u8]) -> (i32, i32, Option<usize>) {
    let mut s = Style::default();
    let (mut w, mut h) = (0, 0);
    loop {
        let (stop, lw, lh) = measure_line(bytes, i, widths, &mut s);
        w = w.max(lw);
        h += lh + LINE_SPACING + s.extra;
        i = stop;
        match bytes.get(i) {
            Some(0x10) => return (w, h - (LINE_SPACING + s.extra), Some(i + 1)),
            None | Some(0x00) => return (w, h - (LINE_SPACING + s.extra), None),
            _ => {}
        }
    }
}

/// FUN_1403aeaa0: the largest page extent over all pages, rounded up to
/// even.
pub fn measure(bytes: &[u8], widths: &[u8]) -> (i32, i32) {
    let (mut w, mut h, mut at) = (0, 0, Some(0));
    while let Some(i) = at {
        let (pw, ph, next) = measure_page(bytes, i, widths);
        w = w.max(pw);
        h = h.max(ph);
        at = next;
    }
    ((w + 1) & !1, (h + 1) & !1)
}

/// One drawn glyph: layout-space rectangle, texel rectangle, RGBA.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glyph {
    pub rect: [f32; 4],
    pub uv: [i32; 4],
    pub rgba: [f32; 4],
}

/// The page cursor and typewriter state of a message (text struct).
#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    pub bytes: Vec<u8>,
    pub page: usize,
    pub complete: bool,
    budget: f32,
    cost: f32,
    fade: f32,
    page_size: (i32, i32),
    pub colour: u32,
}

impl Text {
    pub fn new(bytes: Vec<u8>, colour: u32, widths: &[u8]) -> Self {
        let mut t = Text {
            bytes,
            page: 0,
            complete: false,
            budget: 0.,
            cost: 0.,
            fade: 0.,
            page_size: (0, 0),
            colour,
        };
        t.start_page(0, widths);
        t
    }

    /// FUN_1403b0a30: a page starts with an empty budget and its own extent;
    /// code 0x17 a b sets the per-glyph cost and fade-in.
    fn start_page(&mut self, at: usize, widths: &[u8]) {
        self.page = at;
        self.complete = false;
        self.budget = 0.;
        let (w, h, _) = measure_page(&self.bytes, at, widths);
        self.page_size = (w, h);
        let mut i = at;
        while let Some(&b) = self.bytes.get(i) {
            if b == 0 || b == 0x10 {
                break;
            }
            if b == 0x17 {
                self.cost = self.bytes.get(i + 1).copied().unwrap_or(0) as f32;
                self.fade = self.bytes.get(i + 2).copied().unwrap_or(0) as f32;
            }
            i = next(&self.bytes, i);
        }
    }

    /// FUN_1403b0950: moves past the next page break; false at the end.
    pub fn next_page(&mut self, widths: &[u8]) -> bool {
        let mut i = self.page;
        while let Some(&b) = self.bytes.get(i) {
            match b {
                0x00 => return false,
                0x10 => {
                    self.start_page(i + 1, widths);
                    return true;
                }
                _ => i = next(&self.bytes, i),
            }
        }
        false
    }

    /// FUN_1403afd90 / FUN_1403b0280: lays out this frame's glyphs at
    /// (`x`, `y`) with alignment 3 (centred on the page extent). The budget
    /// grows by one frame; each glyph costs ``cost``. Sets ``complete`` when
    /// the page end is reached.
    pub fn draw(&mut self, x: i32, y: i32, widths: &[u8]) -> Vec<Glyph> {
        self.budget += 1.;
        let (pw, ph) = self.page_size;
        let left = (x - pw / 2) as f32;
        let top = (y - ph / 2) as f32;
        let mut s = Style::default();
        let mut rgba = color(self.colour);
        let mut out = Vec::new();
        let mut i = self.page;
        let mut cursor_y = top;
        let mut budget = self.budget;
        let mut previous = None;
        'lines: loop {
            // Each line starts at the page's left edge one line height down.
            let mut probe = s;
            let (_, _, lh) = measure_line(&self.bytes, i, widths, &mut probe);
            cursor_y += lh as f32;
            let mut cursor_x = left;
            loop {
                if budget < 0. {
                    return out;
                }
                let Some(&b) = self.bytes.get(i) else {
                    self.complete = true;
                    return out;
                };
                match b {
                    0x00 | 0x10 => {
                        self.complete = true;
                        return out;
                    }
                    0x01 => cursor_x += space(&s) as f32,
                    0x02 => {
                        cursor_y += (LINE_SPACING + s.extra) as f32;
                        i = next(&self.bytes, i);
                        continue 'lines;
                    }
                    0x03 => {
                        s.apply(&self.bytes, i);
                        rgba = color(self.colour);
                    }
                    0x07 => {
                        let c = u32::from_le_bytes(
                            self.bytes
                                .get(i + 1..i + 5)
                                .and_then(|w| w.try_into().ok())
                                .unwrap_or([0; 4]),
                        );
                        rgba = color(c);
                    }
                    0x0a..=0x0c => s.apply(&self.bytes, i),
                    0x14 => {
                        let n = i16::from_le_bytes([
                            self.bytes.get(i + 1).copied().unwrap_or(0),
                            self.bytes.get(i + 2).copied().unwrap_or(0),
                        ]);
                        budget -= n as f32;
                    }
                    b if b >= 0x20 => {
                        let mut idx = glyph_index(&self.bytes, i).unwrap_or(0);
                        if idx == 0x46 && previous == Some(0x46) {
                            cursor_x -= 2. * s.scale * s.width_scale;
                            idx = 0x47;
                        }
                        previous = Some(idx);
                        let mut c = rgba;
                        if self.fade > 0. && budget / self.fade < 1. {
                            c[3] *= budget / self.fade;
                        }
                        let bottom = cursor_y + BASELINE;
                        let (u, v) = (
                            (idx % COLUMNS) as i32 * CELL[0] as i32,
                            (idx / COLUMNS) as i32 * CELL[1] as i32,
                        );
                        out.push(Glyph {
                            rect: [
                                cursor_x,
                                bottom - CELL[1] * s.scale,
                                cursor_x + CELL[0] * s.scale * s.width_scale,
                                bottom,
                            ],
                            uv: [u, v, u + CELL[0] as i32, v + CELL[1] as i32],
                            rgba: c,
                        });
                        let width = widths.get(idx).copied().unwrap_or(0);
                        cursor_x += advance(width, &s) as f32;
                        budget -= self.cost;
                    }
                    _ => {}
                }
                i = next(&self.bytes, i);
            }
        }
    }
}

/// Glyph rectangles as canvas quads for the font sheet.
pub fn glyph_quads(glyphs: &[Glyph]) -> Vec<Quad> {
    glyphs
        .iter()
        .map(|g| {
            let [x0, y0, x1, y1] = g.rect;
            Quad {
                corners: [[x0, y0], [x1, y0], [x0, y1], [x1, y1]],
                uv: g.uv,
                colors: [g.rgba; 4],
                blend: 0,
            }
        })
        .collect()
}

/// YS::WINDOW_TALK.
pub struct TalkWindow {
    seq: Sequence,
    box_player: Player,
    tail: Option<Player>,
    arrow: Option<Player>,
    tail_position: u8,
    window_type: usize,
    /// 0 open, 1 text, 2 waiting, 3 done, 4..6 closing, 7 deleted.
    pub state: u8,
    pub text: Text,
    size: (i32, i32),
    position: [i32; 2],
    widths: Vec<u8>,
    glyphs: Vec<Glyph>,
}

impl TalkWindow {
    /// FUN_1403bb700 → FUN_1403bb2d0: box at its open group, tail group 47
    /// (enable 1, kind 1, position 0), text page 0.
    pub fn new(seq: &Sequence, message: Vec<u8>, colour: u32, widths: &[u8]) -> Self {
        let window_type = 0;
        let size = measure(&message, widths);
        let (enable, kind, position) = (1usize, 1usize, 0u8);
        let tail_group = if position > 3 { 0x21 } else { 0x25 } + (kind + enable * 4) * 2;
        TalkWindow {
            seq: seq.clone(),
            box_player: Player::new(seq, BOX_OPEN + 3 * window_type),
            tail: Some(Player::new(seq, tail_group)),
            arrow: None,
            tail_position: position,
            window_type,
            state: 0,
            text: Text::new(message, colour, widths),
            size,
            position: TALK_POSITION,
            widths: widths.to_vec(),
            glyphs: Vec::new(),
        }
    }

    /// YS::WINDOW_BALLOON (FUN_1403bb270: font 3, colour id 5, type 0, no
    /// tail) as the world map's message widget places it (FUN_140393760:
    /// position (0x100, 0x30), alignment 2 = centred horizontally, top at
    /// y) [I: alignment 2 read as the box's top edge].
    pub fn balloon(seq: &Sequence, message: Vec<u8>, colour: u32, widths: &[u8]) -> Self {
        let mut w = Self::new(seq, message, colour, widths);
        w.tail = None;
        w.position = [0x100, 0x30 + w.size.1 / 2];
        w
    }

    pub fn size(&self) -> (i32, i32) {
        self.size
    }

    fn set(player: &mut Player, seq: &Sequence, group: usize) {
        if player.group != group {
            *player = Player::new(seq, group);
        }
    }

    /// FUN_1403bce30 then the draw-time player steps (FUN_1403bbeb0).
    /// `advance` is this frame's trigger & 0x40c. Returns se000 sounds.
    pub fn step(&mut self, advance: bool) -> Vec<u32> {
        let mut sounds = Vec::new();
        let t = self.window_type;
        match self.state {
            0 => {
                if self.box_player.finished() {
                    Self::set(&mut self.box_player, &self.seq, BOX_OPEN + 1 + 3 * t);
                    self.state = 1;
                }
            }
            1 => {
                if self.text.complete {
                    // No choices: mode 1, the advance arrow.
                    self.arrow = Some(Player::new(&self.seq, ARROW));
                    self.state = 2;
                }
            }
            2 => {
                if advance {
                    sounds.push(ADVANCE_SOUND);
                    if self.text.next_page(&self.widths) {
                        self.state = 1;
                    } else {
                        // FUN_1403bcb40: the arrow vanishes; state 3.
                        if let Some(a) = self.arrow.as_mut() {
                            Self::set(a, &self.seq, ARROW_CLOSE);
                        }
                        self.state = 3;
                    }
                }
            }
            4 => {
                if let Some(tail) = self.tail.as_mut() {
                    let close = tail.group + 1;
                    Self::set(tail, &self.seq, close);
                }
                self.state = 5;
            }
            5 => {
                if self.tail.as_ref().is_none_or(|p| p.finished()) {
                    Self::set(&mut self.box_player, &self.seq, BOX_OPEN + 2 + 3 * t);
                    self.state = 6;
                }
            }
            6 if self.box_player.finished() => self.state = 7,
            _ => {}
        }
        // Draw-time steps: box always, tail when state > 0, text in 1..=5,
        // arrow when state > 1.
        let (w, h) = self.size;
        let group = self.box_player.group;
        self.seq.fit_nine_slice(group, w, h);
        self.box_player.update(&self.seq, 1.);
        if self.state > 0
            && let Some(tail) = self.tail.as_mut()
        {
            tail.update(&self.seq, 1.);
        }
        self.glyphs = if (1..=5).contains(&self.state) {
            let [x, y] = self.position;
            self.text.draw(x, y, &self.widths)
        } else {
            Vec::new()
        };
        if self.state > 1
            && let Some(a) = self.arrow.as_mut()
        {
            a.update(&self.seq, 1.);
        }
        sounds
    }

    /// The owner saw state 3 (FUN_1403bc170): start closing.
    pub fn close(&mut self) {
        if self.state == 3 {
            self.state = 4;
        }
    }

    pub fn deleted(&self) -> bool {
        self.state >= 7
    }

    /// FUN_1403bc3b0 / FUN_1403bbeb0: box at the window position, tail at
    /// its anchor, text, then the arrow at (x + w/2, y + h/2).
    pub fn draw(&self, sheet: usize, font_sheet: usize) -> Vec<Draw> {
        let m = Modifiers::default();
        let [x, y] = self.position;
        let (w, h) = self.size;
        let mut out = vec![Draw::Quads {
            quads: self.seq.quads(&self.box_player, [x as f32, y as f32], &m),
            z: 0,
            sheet,
        }];
        if self.state > 0
            && let Some(tail) = &self.tail
        {
            let k = TAIL_FACTORS[(self.tail_position & 3) as usize];
            let ty = if self.tail_position > 3 {
                h / 2
            } else {
                -(h / 2)
            };
            let at = [((w as f32 * k) as i32 + x) as f32, (y + ty) as f32];
            out.push(Draw::Quads {
                quads: self.seq.quads(tail, at, &m),
                z: 0,
                sheet,
            });
        }
        if !self.glyphs.is_empty() {
            out.push(Draw::Quads {
                quads: glyph_quads(&self.glyphs),
                z: 0,
                sheet: font_sheet,
            });
        }
        if self.state > 1
            && let Some(a) = &self.arrow
        {
            let at = [(x + w / 2) as f32, (y + h / 2) as f32];
            out.push(Draw::Quads {
                quads: self.seq.quads(a, at, &m),
                z: 0,
                sheet,
            });
        }
        out
    }
}

/// A libretto talk script: {u16 op, u16 argCount, u32 args[argCount]}
/// until op 0 / count 0 (FUN_1403ec260).
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    /// Show message `id` in a WINDOW_TALK and wait until it is done.
    Message(u32),
    /// Wait `frames` (float).
    Wait(f32),
    /// Raise game signal `n`.
    Signal(u32),
    Stop,
    Other(u16, Vec<u32>),
}

pub fn read_script(bytes: &[u8]) -> Vec<Op> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 4 <= bytes.len() {
        let op = u16::from_le_bytes([bytes[i], bytes[i + 1]]);
        let n = u16::from_le_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
        if op == 0 && n == 0 {
            break;
        }
        let args: Vec<u32> = (0..n)
            .filter_map(|k| bytes.get(i + 4 + 4 * k..i + 8 + 4 * k))
            .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
            .collect();
        out.push(match (op, args.as_slice()) {
            (1, [id, ..]) => Op::Message(*id),
            (2, [f, ..]) => Op::Wait(f32::from_bits(*f)),
            (3, [n, ..]) => Op::Signal(*n),
            (4, _) => Op::Stop,
            _ => Op::Other(op, args),
        });
        i += 4 + 4 * n;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_lengths_follow_the_exe_table() {
        let expect = [
            (0x01, 1),
            (0x04, 2),
            (0x05, 7),
            (0x07, 5),
            (0x08, 4),
            (0x0f, 6),
            (0x10, 1),
            (0x11, 5),
            (0x12, 3),
            (0x13, 5),
            (0x16, 2),
            (0x17, 3),
            (0x19, 2),
        ];
        for (b, n) in expect {
            assert_eq!(code_length(b), n, "{b:#x}");
        }
    }

    #[test]
    fn measures_lines_pages_and_rounds_to_even() {
        // Glyph widths: every glyph 10 px; 'A' = 0x21.
        let widths = [10u8; 200];
        // "AAA{nl}AA{10}A" → page 1: 30 x (24+2+24) = 50; page 2: 10 x 24.
        let text = [0x21, 0x21, 0x21, 0x02, 0x21, 0x21, 0x10, 0x21];
        assert_eq!(measure_page(&text, 0, &widths), (30, 50, Some(7)));
        assert_eq!(measure_page(&text, 7, &widths), (10, 24, None));
        assert_eq!(measure(&text, &widths), (30, 50));
        // A space is a third of the cell: 8 px; odd extents round up.
        assert_eq!(measure(&[0x21, 0x01], &widths), (18, 24));
    }

    #[test]
    fn text_draws_whole_page_at_once_and_pages_on() {
        let widths = [10u8; 200];
        let text = [0x21, 0x02, 0x22, 0x10, 0x23, 0x00];
        let mut t = Text::new(text.to_vec(), 0x80808080, &widths);
        let g = t.draw(256, 336, &widths);
        assert!(t.complete);
        assert_eq!(g.len(), 2);
        // Page 10 x 50 centred: left 251, top 311; line 1 bottom 335 + 2,
        // and the 32-px cell rises from there.
        assert_eq!(g[0].rect, [251., 305., 275., 337.]);
        // Line 2 is 26 px lower; glyph 2 sits in column 2.
        assert_eq!(g[1].rect[3], 363.);
        assert_eq!(g[1].uv, [48, 0, 72, 32]);
        assert!(t.next_page(&widths));
        assert!(!t.complete);
        assert_eq!(t.draw(256, 336, &widths).len(), 1);
        assert!(!t.next_page(&widths));
    }

    #[test]
    fn typewriter_cost_limits_glyphs_per_frame() {
        let widths = [10u8; 200];
        // {17:02 00}AAAA: two frames per glyph.
        let text = [0x17, 0x02, 0x00, 0x21, 0x21, 0x21, 0x21, 0x00];
        let mut t = Text::new(text.to_vec(), 0x80808080, &widths);
        assert_eq!(t.draw(0, 0, &widths).len(), 1);
        assert_eq!(t.draw(0, 0, &widths).len(), 2);
        assert!(!t.complete);
    }

    #[test]
    fn reads_libretto_ops() {
        let mut b = Vec::new();
        for (op, args) in [(1u16, vec![0x31f1u32]), (3, vec![7]), (0, vec![])] {
            b.extend(op.to_le_bytes());
            b.extend((args.len() as u16).to_le_bytes());
            for a in args {
                b.extend(a.to_le_bytes());
            }
        }
        assert_eq!(read_script(&b), vec![Op::Message(0x31f1), Op::Signal(7)]);
    }
}
