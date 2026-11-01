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
//!
//! A big chest's (`dk::TREASURE_EVENT_INFO`, `FUN_140158ee0`, `FUN_140159ae0`,
//! `FUN_140159210`) holds group 0x4d and `ipic` group 2 until confirmed, then
//! closes on 0x4e and 3, with the item's name and description centred.
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
const EVENT_GROUP: usize = 0x4d;
const EVENT_CLOSE: usize = 0x4e;
const EVENT_PICTURE: usize = 2;
const EVENT_PICTURE_CLOSE: usize = 3;
/// se000: the window opens, the confirm.
const SE_OPEN: u32 = 10;
const SE_CONFIRM: u32 = 24;
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

/// A big chest's window.
struct EventSlot {
    name: Vec<u8>,
    description: Vec<u8>,
    picture: Option<Texture>,
    ipic: Option<Sequence>,
    player: Option<Player>,
    picture_player: Option<Player>,
}

#[derive(Default)]
pub struct TreasureInfo {
    prizes: VecDeque<Vec<u8>>,
    current: Option<Slot>,
    boxes: VecDeque<BoxSlot>,
    events: VecDeque<EventSlot>,
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

    /// `FUN_1401595b0` / `FUN_1403a1e80`.
    pub fn push_event(
        &mut self,
        name: Vec<u8>,
        description: Vec<u8>,
        picture: Option<Texture>,
        ipic: Option<Sequence>,
    ) {
        self.events.push_back(EventSlot {
            name,
            description,
            picture,
            ipic,
            player: None,
            picture_player: None,
        });
    }

    /// The image the chest's window shows.
    pub fn picture(&self) -> Option<&Texture> {
        if let Some(e) = self.events.front()
            && e.player.is_some()
        {
            return e.picture.as_ref();
        }
        let b = self.boxes.front()?;
        b.player.as_ref()?;
        b.picture.as_ref()
    }

    /// One tick of a big chest's window; the sounds it starts.
    pub fn update_event(&mut self, seq: &Sequence, confirm: bool) -> Vec<u32> {
        let mut sounds = Vec::new();
        let Some(e) = self.events.front_mut() else {
            return sounds;
        };
        if seq.groups.get(EVENT_CLOSE).is_none() {
            self.events.clear();
            return sounds;
        }
        let Some(player) = e.player.as_mut() else {
            e.player = Some(Player::new(seq, EVENT_GROUP));
            if let Some(ipic) = e
                .ipic
                .as_ref()
                .filter(|s| s.groups.get(EVENT_PICTURE_CLOSE).is_some())
            {
                e.picture_player = Some(Player::new(ipic, EVENT_PICTURE));
            }
            sounds.push(SE_OPEN);
            return sounds;
        };
        if confirm && player.group != EVENT_CLOSE {
            sounds.push(SE_CONFIRM);
            *player = Player::new(seq, EVENT_CLOSE);
            if let (Some(p), Some(ipic)) = (e.picture_player.as_mut(), e.ipic.as_ref()) {
                *p = Player::new(ipic, EVENT_PICTURE_CLOSE);
            }
        }
        player.update(seq, 1.);
        if let (Some(p), Some(ipic)) = (e.picture_player.as_mut(), e.ipic.as_ref()) {
            p.update(ipic, 1.);
        }
        if player.group == EVENT_CLOSE && player.finished() {
            self.events.pop_front();
        }
        sounds
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
        if let Some(e) = self.events.front()
            && let Some(player) = e.player.as_ref()
        {
            let m = Modifiers::default();
            out.push(Draw::Quads {
                quads: seq.quads(player, [0.; 2], &m),
                z: 0,
                sheet,
            });
            let ctx = seq.child_context(player, &m).unwrap_or([0.; 2]);
            if let (Some(p), Some(ipic), Some(picture)) =
                (e.picture_player.as_ref(), e.ipic.as_ref(), picture)
            {
                out.push(Draw::Quads {
                    quads: ipic.quads(p, ctx, &m),
                    z: 0,
                    sheet: picture,
                });
            }
            let alpha = seq.context_color(player, &m).map_or(1., |c| c[3].min(1.));
            if alpha > 0. {
                let g = &seq.groups[player.group];
                let (x, y) = (
                    ctx[0] + g.light_position_x as f32,
                    ctx[1] + g.text_position_y as f32,
                );
                let rgba = [TEXT_RGB[0], TEXT_RGB[1], TEXT_RGB[2], alpha];
                // The name, centred; the description the group's +0x20 below
                // (`FUN_140159210`), in the lower box.
                let name = text_quads(&e.name, [x, y], TEXT_SIZE, 2, 0., rgba, false, font);
                let below = y + g.text_position_x as f32;
                let text = text_quads(
                    &e.description,
                    [x, below],
                    TEXT_SIZE,
                    2,
                    0.,
                    rgba,
                    false,
                    font,
                );
                for q in [name, text] {
                    out.push(Draw::Quads {
                        quads: q.glyphs,
                        z: 0,
                        sheet: font_sheet,
                    });
                }
            }
            return out;
        }
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
