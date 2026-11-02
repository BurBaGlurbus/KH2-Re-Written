//! The Gummi flight's result panel (gb::MENU_RESULT, FUN_1402378a0) on
//! `gumibattle/sprite/<lang>/sprite.bar`, a 2LD whose SEQD k draws from image
//! k. Every sprite is a player on one SEQD group: its appear, then its loop
//! once the appear has ended. The right column (this run) sits 85 units right
//! of the layout, the left column (the saved record) 85 left. [Not built] The
//! unlock notices (advice.bin), the prizes and the HUD item window, the
//! Treasure List prompt and panel, "COMPLETION BONUS!".
use crate::field_command_menu::Font;
use crate::sprite_player::{self, Id, Sprites, Text};
use crate::world_map_menu::Sheet;
use data::sprite_sequence::{Quad, Sequence};
use gummi::Mission;

/// The columns' shift (0x55), the 4:3 layout in a 16:9 frame.
const SHIFT: f32 = 85.;
/// The banner holds this long before the columns (DAT_1405b6274).
const BANNER_FRAMES: u32 = 210;

/// One sprite: its SEQD, player id and the loop to switch to.
struct Part {
    seq: usize,
    id: Id,
}

/// A digit row: its sprites (most significant first), the value's digits
/// and how many are shown.
struct Row {
    digits: Vec<Part>,
    groups: [i32; 11],
    value: Vec<u32>,
    shown: usize,
    every: u32,
    placeholder: Option<i32>,
    /// The new-record mark shown once every digit is (0x7c..0x7e).
    mark: Option<i32>,
}

/// What the panel asks of its host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResultEvent {
    Sound(u32),
}

/// A mission's result: this run, the saved record before it, and what the
/// level ranks (rank.bin kind: 0 medals, 1 ships destroyed, 2 score).
pub struct ResultInput {
    pub route_name: Vec<u8>,
    pub level: u8,
    pub ex: bool,
    pub kind: u8,
    pub run: Mission,
    pub saved: Mission,
    /// A route's first level 1 clear ("ROUTE OPENED", no left column).
    pub first: bool,
}

pub struct ResultPanel {
    sprites: Vec<Sprites>,
    input: ResultInput,
    timer: u32,
    state: u8,
    rows: Vec<(u32, Row)>,
    /// Timed parts still to start: (frame, seq, appear, loop, column, sound).
    pending: Vec<(u32, usize, i32, i32, f32, Option<u32>)>,
    pub done: bool,
}

fn g(v: i32) -> Option<usize> {
    usize::try_from(v).ok()
}

fn digits(mut v: u32, slots: usize) -> Vec<u32> {
    let mut out = vec![0; slots];
    for d in out.iter_mut().rev() {
        *d = v % 10;
        v /= 10;
    }
    out
}

impl ResultPanel {
    pub fn new(input: ResultInput) -> Self {
        Self {
            sprites: Vec::new(),
            input,
            timer: 0,
            state: 0,
            rows: Vec::new(),
            pending: Vec::new(),
            done: false,
        }
    }

    fn sprite(
        &mut self,
        seqs: &[Sequence],
        seq: usize,
        appear: i32,
        lp: i32,
        x: f32,
        text: Option<Text>,
    ) -> Option<Part> {
        let s = seqs.get(seq)?;
        let first = g(appear).or(g(lp))?;
        if self.sprites.len() <= seq {
            self.sprites.resize_with(seq + 1, Sprites::default);
        }
        let id = self.sprites[seq].create(s, 1, seq, first, g(lp).filter(|&l| l != first), text);
        self.sprites[seq].get_mut(id).base = [x, 0.];
        Some(Part { seq, id })
    }

    /// Digits after a label (FUN_14023aaa0): from the label's anchor plus
    /// its loop group's +0x10/+0x14, left to right, each digit group's
    /// +0x1c apart; leading zeros blank.
    #[allow(clippy::too_many_arguments)]
    fn row(
        &mut self,
        seqs: &[Sequence],
        label: &Part,
        lp: i32,
        groups: [i32; 11],
        value: u32,
        slots: usize,
        placeholder: Option<i32>,
    ) -> Row {
        let seq = &seqs[label.seq];
        let lg = g(lp).and_then(|k| seq.groups.get(k));
        let (lx, ty) = lg.map_or((0., 0.), |k| {
            (k.light_position_x as f32, k.text_position_y as f32)
        });
        let every = lg.map_or(8., |k| (k.text_position_x as f32).max(2.)) as u32;
        let value = digits(value, slots);
        let mut x = lx;
        let mut parts = Vec::new();
        for k in 0..slots {
            let first = placeholder.or(Some(groups[10])).unwrap_or(-1);
            let Some(gr) = g(first) else { continue };
            let s = &mut self.sprites[label.seq];
            let id = s.create(seq, 2, label.seq, gr, None, None);
            s.attach(id, label.id);
            let d = s.get_mut(id);
            d.offset = [x, ty];
            d.inherit_alpha = true;
            let adv = g(groups[value[k] as usize])
                .and_then(|k| seq.groups.get(k))
                .map_or(0., |k| k.ui_padding as f32);
            x += adv;
            parts.push(Part { seq: label.seq, id });
        }
        Row {
            digits: parts,
            groups,
            value,
            shown: 0,
            every,
            placeholder,
            mark: None,
        }
    }

    /// Shows the least significant `n` digits; leading zeros blank.
    fn reveal(sprites: &mut [Sprites], seqs: &[Sequence], row: &mut Row, n: usize) {
        let slots = row.value.len();
        let lead = row.value.iter().take_while(|&&d| d == 0).count();
        for k in 0..slots {
            let from_right = slots - 1 - k;
            let group = if from_right >= n {
                row.placeholder.unwrap_or(row.groups[10])
            } else if k < lead && k + 1 < slots {
                row.groups[10]
            } else {
                row.groups[row.value[k] as usize]
            };
            let p = &row.digits[k];
            if let Some(gr) = g(group) {
                sprites[p.seq].set(&seqs[p.seq], p.id, gr, None);
            }
        }
        row.shown = n;
    }

    fn build_left(&mut self, seqs: &[Sequence]) {
        let i = &self.input;
        let (saved, level, ex, kind) = (i.saved, i.level, i.ex, i.kind);
        let size = seqs
            .get(7)
            .and_then(|s| s.groups.get(5))
            .map_or(16., |k| k.text_scale as f32);
        let mut t = Text::new(i.route_name.clone(), size);
        t.align = 2;
        t.group_position = false;
        t.keep_rgb = false;
        self.sprite(seqs, 7, 4, 5, -SHIFT, Some(t));
        if ex {
            self.sprite(seqs, 7, 0xa8, 0xa9, -SHIFT, None);
        }
        self.sprite(seqs, 7, 1, 2, -SHIFT, None);
        let lv = [0x2a, 0x2d, 0x30][(level.clamp(1, 3) - 1) as usize];
        self.sprite(seqs, 7, lv, lv + 1, -SHIFT, None);
        self.sprite(seqs, 7, 0x9d, 0x9e, -SHIFT, None);
        if let Some(label) = self.sprite(seqs, 7, 0x65, 0x66, -SHIFT, None) {
            let letter = match saved.rank {
                r @ 0..=5 => 0x45 - r as i32,
                6 => 0x46,
                r @ 7..=15 => 0x48 + (r as i32 - 7),
                _ => 0x47,
            };
            let s = &mut self.sprites[7];
            let id = s.create(&seqs[7], 2, 7, letter as usize, None, None);
            s.attach(id, label.id);
            s.get_mut(id).offset = [50., 0.];
        }
        let (appear, lp, groups, value, slots): (i32, i32, std::ops::Range<i32>, u32, usize) =
            match kind {
                0 => (0x62, 0x63, 0x35..0x40, u32::from(saved.medals), 2),
                1 => (0x5f, 0x60, 0x1e..0x29, u32::from(saved.destroyed()), 4),
                _ => (
                    0x68,
                    0x69,
                    0x51..0x5c,
                    saved.score,
                    if saved.score > 9_999_990 { 8 } else { 7 },
                ),
            };
        if let Some(label) = self.sprite(seqs, 7, appear, lp, -SHIFT, None) {
            let gs: Vec<i32> = groups.collect();
            let mut row = self.row(seqs, &label, lp, gs.try_into().unwrap(), value, slots, None);
            let n = row.value.len();
            Self::reveal(&mut self.sprites, seqs, &mut row, n);
        }
    }

    fn schedule_right(&mut self) {
        let run = self.input.run;
        // (frame, seq, appear, loop, column, sound)
        self.pending.push((0, 7, 0x9f, 0xa0, SHIFT, Some(0xf372e)));
        self.pending.push((18, 7, 0xa1, 0xa2, SHIFT, None));
        let (a, l) = if run.score <= 9_999_990 {
            (0xa3, 0xa4)
        } else {
            (0xad, 0xae)
        };
        self.pending.push((36, 7, a, l, SHIFT, None));
        let rank = match run.rank {
            0 => None,
            r @ 1..=5 => Some((0xe - 2 * (r as i32 - 1), 0xf - 2 * (r as i32 - 1))),
            _ => Some((0x1d, 0x1e)),
        };
        if let Some((a, l)) = rank {
            self.pending.push((60, 6, a, l, -SHIFT, Some(0xf3735)));
            if self.input.saved.rank < run.rank {
                self.pending.push((300, 7, 0x7f, 0x7f, -SHIFT, None));
            }
        }
    }

    /// One frame; `decide` shows everything at once (timer 1000).
    pub fn tick(&mut self, seqs: &[Sequence], decide: bool) -> Vec<ResultEvent> {
        let mut out = Vec::new();
        match self.state {
            // A record whose rank went up before skips the banner (state 1,
            // asm 0x140237917) [U: intended?].
            0 if self.input.saved.rank_up => {
                self.timer = BANNER_FRAMES;
                self.state = 1;
            }
            0 => {
                let b = if self.input.first {
                    2
                } else if !self.input.run.rank_up {
                    1
                } else {
                    0
                };
                out.push(ResultEvent::Sound([0xf3732, 0xf3737, 0xf372d][b]));
                self.sprite(seqs, 4, b as i32, -1, 0., None);
                self.state = 1;
            }
            1 => {
                self.timer += 1;
                if self.timer >= BANNER_FRAMES {
                    if !self.input.first {
                        out.push(ResultEvent::Sound(0xf372c));
                        self.build_left(seqs);
                    }
                    self.schedule_right();
                    self.timer = 0;
                    self.state = 2;
                }
            }
            2 => {
                if decide {
                    self.timer = 1000;
                }
                let now = self.timer;
                let due: Vec<_> = self
                    .pending
                    .iter()
                    .copied()
                    .filter(|p| p.0 <= now)
                    .collect();
                self.pending.retain(|p| p.0 > now);
                let run = self.input.run;
                for (_, seq, a, l, x, sound) in due {
                    out.extend(sound.map(ResultEvent::Sound));
                    let Some(label) = self.sprite(seqs, seq, a, l, x, None) else {
                        continue;
                    };
                    let row = match (seq, a) {
                        // The rank's new-record mark (0x7f) has no row.
                        (7, 0x7f) => continue,
                        (7, 0x9f) => {
                            // The medal emblem (SEQD 0, 3v + 1) and two digits,
                            // whose row the exe places at anchor.y + the group's
                            // +0x10 (sic, FUN_14023aaa0), not +0x14.
                            let v = u32::from(run.medals).min(30);
                            self.sprite(seqs, 0, 3 * v as i32 + 1, -1, SHIFT, None);
                            let gs: Vec<i32> = (0x35..0x40).collect();
                            let mut r =
                                self.row(seqs, &label, l, gs.try_into().unwrap(), v, 2, None);
                            let lx = g(l)
                                .and_then(|k| seqs[7].groups.get(k))
                                .map_or(0., |k| k.light_position_x as f32);
                            for p in &r.digits {
                                self.sprites[p.seq].get_mut(p.id).offset[1] = lx;
                            }
                            Self::reveal(&mut self.sprites, seqs, &mut r, 2);
                            if self.input.saved.medals < run.medals {
                                self.sprite(seqs, 7, 0x7c, 0x7c, SHIFT, None);
                            }
                            r
                        }
                        (7, 0xa1) => {
                            let gs: Vec<i32> = (6..0x11).collect();
                            let v = u32::from(run.destroyed());
                            let mut r =
                                self.row(seqs, &label, l, gs.try_into().unwrap(), v, 4, Some(0x11));
                            if self.input.saved.destroyed() < run.destroyed() {
                                r.mark = Some(0x7d);
                            }
                            r
                        }
                        (7, _) => {
                            let gs: Vec<i32> = (0x12..0x1d).collect();
                            let slots = if run.score > 9_999_990 { 8 } else { 7 };
                            let mut r = self.row(
                                seqs,
                                &label,
                                l,
                                gs.try_into().unwrap(),
                                run.score,
                                slots,
                                Some(0x1d),
                            );
                            if self.input.saved.score < run.score {
                                r.mark = Some(0x7e);
                            }
                            r
                        }
                        _ => {
                            // Ranks 7..16: the number over the S.
                            if run.rank >= 7 {
                                let n = 0x12 + (run.rank as i32 - 7).min(9);
                                self.sprite(seqs, 6, n, -1, -SHIFT, None);
                            }
                            continue;
                        }
                    };
                    self.rows.push((now, row));
                }
                // One more digit every `every` frames; a row's new-record mark
                // once all are shown.
                let mut marks = Vec::new();
                for (start, row) in &mut self.rows {
                    let want = if now >= 1000 {
                        row.value.len()
                    } else {
                        ((now - *start) / row.every.max(1)) as usize + 1
                    }
                    .min(row.value.len());
                    if want > row.shown {
                        Self::reveal(&mut self.sprites, seqs, row, want);
                        if want == row.value.len() {
                            marks.extend(row.mark.take());
                        }
                    }
                }
                for m in marks {
                    self.sprite(seqs, 7, m, m, SHIFT, None);
                }
                if self.pending.is_empty()
                    && self.rows.iter().all(|(_, r)| r.shown == r.value.len())
                {
                    self.done = true;
                }
                self.timer = self.timer.saturating_add(1);
            }
            _ => {}
        }
        for (k, s) in self.sprites.iter_mut().enumerate() {
            if let Some(seq) = seqs.get(k) {
                s.update(seq);
            }
        }
        out
    }

    /// The result's level, EX flag and run block.
    pub fn input(&self) -> (u8, bool, Mission) {
        (self.input.level, self.input.ex, self.input.run)
    }

    /// A route's first level 1 clear (no after-clear menu follows).
    pub fn first(&self) -> bool {
        self.input.first
    }

    pub fn draw(&self, seqs: &[Sequence], font: &Font) -> Vec<(Sheet, Vec<Quad>)> {
        let mut out = Vec::new();
        for (k, s) in self.sprites.iter().enumerate() {
            for (sheet, quads) in s.draw(&seqs[k], font) {
                out.push((
                    match sheet {
                        sprite_player::Sheet::Art(i) => Sheet::Image(i),
                        sprite_player::Sheet::Font => Sheet::Font,
                        sprite_player::Sheet::Icon => Sheet::Icon,
                    },
                    quads,
                ));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digits_most_significant_first() {
        assert_eq!(digits(907, 4), vec![0, 9, 0, 7]);
        assert_eq!(digits(12345678, 7), vec![2, 3, 4, 5, 6, 7, 8]);
    }
}
