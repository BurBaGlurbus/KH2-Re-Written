//! dk::TREASURE_INFO (`FUN_140159320`, per frame `FUN_140159c60`): the windows
//! the field shows for an obtained item. One at a time per queue, each after
//! the last, with no pause and no button.
//!
//! A picked up box's name (`dk::TREASURE_PRIZE_INFO`, `FUN_1401595d0`,
//! `FUN_1401596c0`) plays `gaug` group 0x4c. Text is `09 icon name`
//! (`FUN_1403c4330`). Starting one plays se000 #10 (`FUN_140159ed0`).
use crate::field_command_menu::Font;
use crate::gauge::Draw;
use crate::sprite_player::text_quads;
use data::sprite_sequence::{Modifiers, Player, Sequence};
use std::collections::VecDeque;

/// `FUN_1401595d0`: the prize ring holds 16, refused when 15 wait.
const PRIZE_WAITING: usize = 15;
const PRIZE_GROUP: usize = 0x4c;
/// `FUN_140158de0`: text size +0x20c, colour +0x208 (RGB; alpha from the group).
const TEXT_SIZE: f32 = 16.;
const TEXT_RGB: [f32; 3] = [229. / 128.; 3];

struct Slot {
    text: Vec<u8>,
    player: Player,
}

impl Slot {
    fn new(seq: &Sequence, text: Vec<u8>) -> Self {
        Slot {
            text,
            player: Player::new(seq, PRIZE_GROUP),
        }
    }
}

#[derive(Default)]
pub struct TreasureInfo {
    prizes: VecDeque<Vec<u8>>,
    current: Option<Slot>,
}

impl TreasureInfo {
    /// `FUN_1401595d0`.
    pub fn push_prize(&mut self, text: Vec<u8>) {
        if self.prizes.len() < PRIZE_WAITING {
            self.prizes.push_back(text);
        }
    }

    /// One tick; true when a window starts (`FUN_1401dd170(10)`).
    pub fn update(&mut self, seq: &Sequence) -> bool {
        if seq.groups.get(PRIZE_GROUP).is_none() {
            self.prizes.clear();
            return false;
        }
        if let Some(s) = self.current.as_mut() {
            s.player.update(seq, 1.);
            if s.player.finished() {
                self.current = None;
            }
        }
        if self.current.is_none()
            && let Some(text) = self.prizes.pop_front()
        {
            self.current = Some(Slot::new(seq, text));
            return true;
        }
        false
    }

    /// The window's sprites, then its text (`FUN_14016ae30`), which takes the
    /// group's anchor and alpha. `sheet` is the gauge sheet, `font_sheet` the
    /// system font's and the next one its icons.
    pub fn draw(&self, seq: &Sequence, font: &Font, sheet: usize, font_sheet: usize) -> Vec<Draw> {
        let mut out = Vec::new();
        let Some(s) = self.current.as_ref() else {
            return out;
        };
        let m = Modifiers::default();
        out.push(Draw::Quads {
            quads: seq.quads(&s.player, [0.; 2], &m),
            z: 0,
            sheet,
        });
        let alpha = seq
            .context_color(&s.player, &m)
            .map_or(1., |c| c[3].min(1.));
        if alpha <= 0. {
            return out;
        }
        let g = &seq.groups[s.player.group];
        let ctx = seq.child_context(&s.player, &m).unwrap_or([0.; 2]);
        let at = [
            ctx[0] + g.light_position_x as f32,
            ctx[1] + g.text_position_y as f32,
        ];
        let rgba = [TEXT_RGB[0], TEXT_RGB[1], TEXT_RGB[2], alpha];
        let q = text_quads(&s.text, at, TEXT_SIZE, 0, 0., rgba, false, font);
        out.push(Draw::Quads {
            quads: q.icons,
            z: 0,
            sheet: font_sheet + 1,
        });
        out.push(Draw::Quads {
            quads: q.glyphs,
            z: 0,
            sheet: font_sheet,
        });
        out
    }
}
