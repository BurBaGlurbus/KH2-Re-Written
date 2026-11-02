//! The Gummi flight's three-row system menus on `file/<lang>/gm_ps.2ld`
//! (menu file 5): after a clear (0x35: Retry / Gummi Menu / World Map) and
//! after a game over (0x34: Continue / Gummi Menu / World Map). They are the
//! title's list framework (FUN_1402f7720, input FUN_1402f9440): a black
//! backdrop (SEQD 1 group 0), each row a sprite on its idle group with its
//! text at the group's anchor, the cursor row on its selected group. Up and
//! down wrap on a fresh press; there is no cancel; decide gives 1, 2 or 3
//! (FUN_1402f0a20).
use crate::field_command_menu::Font;
use crate::sprite_player::{self, Id, Sprites, Text};
use crate::world_map_menu::Sheet;
use data::sprite_sequence::{Quad, Sequence};
use gummi::menu::{self as rules, Step};

/// The menus' SEQD; both of the file's SEQDs draw from its one image.
const SEQ: usize = 1;
const IMAGE: usize = 0;
const BACKDROP: usize = 0;
const IDLE: [usize; 3] = [0x18, 0x19, 0x1a];
const SELECTED: [usize; 3] = [0x0b, 0x0c, 0x0d];
/// System sound requests (FUN_1402eb740): open 0xb, cursor 1, decide 2,
/// se000 #3 / #1 / #2 as in the field menus.
pub mod sound {
    pub const OPEN: u32 = 3;
    pub const MOVE: u32 = 1;
    pub const DECIDE: u32 = 2;
}

/// Which menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// 0x35, texts 0x4abc..0x4abe (table 0x1405baa00).
    AfterClear,
    /// 0x34, texts 0x37d1..0x37d3 (table 0x1405ba9f8).
    GameOver,
}

impl Kind {
    pub fn texts(self) -> [u32; 3] {
        match self {
            Kind::AfterClear => [0x4abc, 0x4abd, 0x4abe],
            Kind::GameOver => [0x37d1, 0x37d2, 0x37d3],
        }
    }
}

pub struct ListMenu {
    sprites: Sprites,
    rows: [Id; 3],
    pub cursor: usize,
    /// 1 retry / continue, 2 the Gummi menu, 3 the world map.
    pub result: Option<u8>,
    opened: bool,
}

impl ListMenu {
    /// `texts` are the rows' messages.
    pub fn new(seqs: &[Sequence], texts: [Vec<u8>; 3]) -> Self {
        let seq = &seqs[SEQ];
        let mut sprites = Sprites::default();
        sprites.create(seq, 0x7533, IMAGE, BACKDROP, None, None);
        let rows = std::array::from_fn(|k| {
            let group = if k == 0 { SELECTED[k] } else { IDLE[k] };
            let size = seq.groups.get(group).map_or(15., |g| g.text_scale as f32);
            let mut t = Text::new(texts[k].clone(), if size > 0. { size } else { 15. });
            t.align = 2;
            t.group_position = false;
            t.keep_rgb = false;
            sprites.create(seq, 0x7534, IMAGE, group, None, Some(t))
        });
        Self {
            sprites,
            rows,
            cursor: 0,
            result: None,
            opened: false,
        }
    }

    /// One frame; returns the sound requests.
    pub fn tick(&mut self, seqs: &[Sequence], pressed: u64, repeat: u64) -> Vec<u32> {
        let seq = &seqs[SEQ];
        let mut out = Vec::new();
        if !self.opened {
            self.opened = true;
            out.push(sound::OPEN);
        } else if self.result.is_none() {
            let before = self.cursor;
            match rules::selector(&mut self.cursor, 3, pressed, repeat) {
                Step::Moved => {
                    out.push(sound::MOVE);
                    self.sprites.set(seq, self.rows[before], IDLE[before], None);
                    self.sprites
                        .set(seq, self.rows[self.cursor], SELECTED[self.cursor], None);
                }
                Step::Decide(row) => {
                    out.push(sound::DECIDE);
                    self.result = Some(row as u8 + 1);
                }
                // No cancel on these menus.
                Step::Cancel | Step::None => {}
            }
        }
        self.sprites.update(seq);
        out
    }

    pub fn draw(&self, seqs: &[Sequence], font: &Font) -> Vec<(Sheet, Vec<Quad>)> {
        self.sprites
            .draw(&seqs[SEQ], font)
            .into_iter()
            .map(|(sheet, quads)| {
                (
                    match sheet {
                        sprite_player::Sheet::Art(i) => Sheet::Image(i),
                        sprite_player::Sheet::Font => Sheet::Font,
                        sprite_player::Sheet::Icon => Sheet::Icon,
                    },
                    quads,
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_texts() {
        assert_eq!(Kind::AfterClear.texts(), [0x4abc, 0x4abd, 0x4abe]);
        assert_eq!(Kind::GameOver.texts(), [0x37d1, 0x37d2, 0x37d3]);
    }
}
