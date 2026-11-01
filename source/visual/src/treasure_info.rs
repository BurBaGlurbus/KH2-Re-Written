//! dk::TREASURE_INFO (`FUN_140159320`, per frame `FUN_140159c60`): the windows
//! the field shows for an obtained item. One at a time per queue, each after
//! the last, with no pause and no button.
//!
//! A picked up box's name (`dk::TREASURE_PRIZE_INFO`, `FUN_1401595d0`,
//! `FUN_1401596c0`) plays `gaug` group 0x4c. Text is `09 icon name`
//! (`FUN_1403c4330`). Starting one plays se000 #10 (`FUN_140159ed0`).
//!
//! A small chest's (`dk::TREASURE_BOX_INFO`, `FUN_140159550`, `FUN_140159970`)
//! waits 60 ticks, then plays `gaug` group 0x4f with the item's name alone and
//! its picture (`ipic` group 6), both 85 to the left.
use crate::field_command_menu::Font;
use crate::gauge::Draw;
use crate::menu_screen::Texture;
use crate::sprite_player::text_quads;
use data::sprite_sequence::{Modifiers, Player, Sequence};
use std::collections::VecDeque;

/// `FUN_1401595d0`: the prize ring holds 16, refused when 15 wait.
const PRIZE_WAITING: usize = 15;
const PRIZE_GROUP: usize = 0x4c;
/// `FUN_140159550`: 7 waiting at most; `FUN_140158de0(.., 0x4f, .., 60., -85)`.
const BOX_WAITING: usize = 7;
const BOX_GROUP: usize = 0x4f;
const BOX_DELAY: f32 = 60.;
const BOX_X: f32 = -85.;
const PICTURE_GROUP: usize = 6;
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

/// A chest's window, waiting or playing.
struct BoxSlot {
    text: Vec<u8>,
    picture: Option<Texture>,
    ipic: Option<Sequence>,
    /// Starts when `delay` runs out.
    delay: f32,
    player: Option<Player>,
    picture_player: Option<Player>,
}

#[derive(Default)]
pub struct TreasureInfo {
    prizes: VecDeque<Vec<u8>>,
    current: Option<Slot>,
    boxes: VecDeque<BoxSlot>,
}

impl TreasureInfo {
    /// `FUN_1401595d0`.
    pub fn push_prize(&mut self, text: Vec<u8>) {
        if self.prizes.len() < PRIZE_WAITING {
            self.prizes.push_back(text);
        }
    }

    /// `FUN_140159550`.
    pub fn push_box(&mut self, text: Vec<u8>, picture: Option<Texture>, ipic: Option<Sequence>) {
        if self.boxes.len() < BOX_WAITING {
            // A window that waits behind another starts straight after it.
            let delay = if self.boxes.is_empty() { BOX_DELAY } else { 0. };
            self.boxes.push_back(BoxSlot {
                text,
                picture,
                ipic,
                delay,
                player: None,
                picture_player: None,
            });
        }
    }

    /// The image the chest's window shows.
    pub fn picture(&self) -> Option<&Texture> {
        let b = self.boxes.front()?;
        b.player.as_ref()?;
        b.picture.as_ref()
    }

    /// `FUN_140159970`: true when the window starts.
    fn update_box(&mut self, seq: &Sequence) -> bool {
        let Some(b) = self.boxes.front_mut() else {
            return false;
        };
        if seq.groups.get(BOX_GROUP).is_none() {
            self.boxes.clear();
            return false;
        }
        if b.delay > 0. {
            b.delay -= 1.;
            return false;
        }
        let Some(player) = b.player.as_mut() else {
            b.player = Some(Player::new(seq, BOX_GROUP));
            if let Some(ipic) = b
                .ipic
                .as_ref()
                .filter(|s| s.groups.get(PICTURE_GROUP).is_some())
            {
                b.picture_player = Some(Player::new(ipic, PICTURE_GROUP));
            }
            return true;
        };
        player.update(seq, 1.);
        if let (Some(p), Some(ipic)) = (b.picture_player.as_mut(), b.ipic.as_ref()) {
            p.update(ipic, 1.);
        }
        if player.finished() {
            self.boxes.pop_front();
        }
        false
    }

    /// One tick; true when a window starts (`FUN_1401dd170(10)`).
    pub fn update(&mut self, seq: &Sequence) -> bool {
        let started = self.update_box(seq);
        self.update_prize(seq) || started
    }

    fn update_prize(&mut self, seq: &Sequence) -> bool {
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
    pub fn draw(
        &self,
        seq: &Sequence,
        font: &Font,
        sheet: usize,
        font_sheet: usize,
        picture: Option<usize>,
    ) -> Vec<Draw> {
        let mut out = Vec::new();
        if let Some(b) = self.boxes.front()
            && let Some(player) = b.player.as_ref()
        {
            let m = Modifiers::default();
            out.push(Draw::Quads {
                quads: seq.quads(player, [BOX_X, 0.], &m),
                z: 0,
                sheet,
            });
            let ctx = seq.child_context(player, &m).unwrap_or([0.; 2]);
            if let (Some(p), Some(ipic), Some(picture)) =
                (b.picture_player.as_ref(), b.ipic.as_ref(), picture)
            {
                out.push(Draw::Quads {
                    quads: ipic.quads(p, [ctx[0] + BOX_X, ctx[1]], &m),
                    z: 0,
                    sheet: picture,
                });
            }
            let alpha = seq.context_color(player, &m).map_or(1., |c| c[3].min(1.));
            if alpha > 0. {
                let g = &seq.groups[player.group];
                let at = [
                    ctx[0] + g.light_position_x as f32 + BOX_X,
                    ctx[1] + g.text_position_y as f32,
                ];
                let rgba = [TEXT_RGB[0], TEXT_RGB[1], TEXT_RGB[2], alpha];
                let q = text_quads(&b.text, at, TEXT_SIZE, 0, 0., rgba, false, font);
                out.push(Draw::Quads {
                    quads: q.glyphs,
                    z: 0,
                    sheet: font_sheet,
                });
            }
        }
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
