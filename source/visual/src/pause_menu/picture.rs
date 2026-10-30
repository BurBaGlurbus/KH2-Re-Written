//! The item picture of the Items and Equip screens: setup FUN_1403022b0 /
//! FUN_1403022a0, request FUN_140302370, close FUN_140302040, kill
//! FUN_140302200, reset FUN_140302280, the timers FUN_140301cc0 /
//! FUN_140301c80 and the show FUN_140301d80. The image is
//! `itempic/item-%03d.imd` (an IMGD), drawn with the `ipic` SEQD of
//! 03system.bin (FUN_1403b0c90); the caller loads the image into the cache.
use crate::sprite_player::{Id, Sprites};
use data::sprite_sequence::{Modifiers, Player, Quad, Sequence};
use std::collections::HashSet;

/// FUN_140302370's timers (0x140623bd4 = 60.0, 0x140623bc0 = 15.0).
const HOLD: f32 = 60.;
const DELAY: f32 = 15.;
/// The picture's show and close groups (default table 0x1405bace0).
const SHOW: usize = 0;
const CLOSE: usize = 1;

/// The picture sprite (DAT_140beeb68): an `ipic` player on the image.
#[derive(Clone, Debug)]
pub struct PictureSprite {
    pub image: u16,
    pub player: Player,
    pub parent: Id,
    /// +0x1d0: the parent group's TextPositionX / 100.
    pub scale: f32,
    pub prio: u32,
    die_when_done: bool,
    pub dead: bool,
}

/// The picture state (DAT_140beeb50..8c, DAT_140743e88..98).
#[derive(Clone, Debug, Default)]
pub struct Picture {
    prio: u32,
    /// Frame groups {appear, close, idle} when the picture makes its own
    /// frame (DAT_140743e96), else the parent is a screen's holder.
    frame_groups: Option<[usize; 3]>,
    /// DAT_140beeb60: the frame or the holder.
    frame: Option<Id>,
    pub sprite: Option<PictureSprite>,
    hold: f32,
    delay: f32,
    ready: bool,
    /// DAT_140743e94: the picture id to show.
    pending: Option<u16>,
    /// The show and close groups a caller gave (else the default table).
    groups: Option<[usize; 2]>,
}

/// The images the caller has loaded (FUN_1403018e0's cache) and the one
/// it should load next.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PictureCache {
    pub loaded: HashSet<u16>,
    pub wanted: Option<u16>,
}

impl Picture {
    /// FUN_1403022b0 (sequence 1, image 0, mode 0).
    pub fn setup(&mut self, prio: u32, frame_groups: Option<[usize; 3]>) {
        self.prio = prio;
        self.frame_groups = frame_groups;
        self.frame = None;
    }

    /// FUN_1403022b0's table argument.
    pub fn set_groups(&mut self, groups: [usize; 2]) {
        self.groups = Some(groups);
    }

    fn close_group(&self) -> usize {
        self.groups.map_or(CLOSE, |g| g[1])
    }

    /// FUN_1403022a0: the picture goes on a screen's holder.
    pub fn set_parent(&mut self, holder: Id) {
        self.frame = Some(holder);
        self.frame_groups = None;
    }

    /// FUN_140302280.
    pub fn reset(&mut self) {
        self.ready = false;
        self.hold = 0.;
        self.pending = None;
    }

    fn own_frame(&self, sp: &Sprites) -> Option<Id> {
        self.frame_groups?;
        self.frame.filter(|&f| sp.alive(f))
    }

    fn close_shown(&mut self, seq: &Sequence, ipic: &Sequence, sp: &mut Sprites) -> bool {
        let mut closed = false;
        if let (Some(f), Some(g)) = (self.own_frame(sp), self.frame_groups)
            && sp.get(f).player.group != g[1]
        {
            sp.close(seq, f, g[1]);
            closed = true;
        }
        let close = self.close_group();
        if let Some(s) = self.sprite.as_mut().filter(|s| !s.dead)
            && s.player.group != close
        {
            s.player = Player::new(ipic, close);
            s.die_when_done = true;
            closed = true;
        }
        closed
    }

    /// FUN_140302370: close what shows and wait 15 frames for `id`.
    pub fn request(&mut self, seq: &Sequence, ipic: &Sequence, sp: &mut Sprites, id: u16) {
        self.hold = HOLD;
        self.delay = DELAY;
        self.ready = false;
        self.close_shown(seq, ipic, sp);
        self.pending = Some(id);
    }

    /// FUN_140302040: close what shows (forgetting the request).
    pub fn close(&mut self, seq: &Sequence, ipic: &Sequence, sp: &mut Sprites) {
        if self.close_shown(seq, ipic, sp) {
            self.pending = None;
        }
    }

    /// FUN_140302200: frame and picture go now.
    pub fn kill(&mut self, sp: &mut Sprites) {
        if let Some(f) = self.own_frame(sp) {
            sp.kill(f);
        }
        if let Some(s) = self.sprite.as_mut() {
            s.dead = true;
        }
        self.reset();
    }

    pub fn alive(&self) -> bool {
        self.sprite.as_ref().is_some_and(|s| !s.dead)
    }

    /// FUN_140301cc0 + FUN_140301c80 (substate 1): after the delay the
    /// image is requested; once it is in the cache it shows.
    pub fn timers(
        &mut self,
        seq: &Sequence,
        ipic: &Sequence,
        sp: &mut Sprites,
        cache: &mut PictureCache,
    ) {
        if self.hold > 0. {
            self.hold = (self.hold - 1.).max(0.);
            self.delay -= 1.;
            if self.delay <= 0. {
                if let Some(id) = self.pending
                    && !cache.loaded.contains(&id)
                {
                    cache.wanted = Some(id);
                }
                self.ready = true;
                self.hold = 0.;
            }
        }
        if self.ready
            && let Some(id) = self.pending
            && cache.loaded.contains(&id)
        {
            self.show(seq, ipic, sp, id);
            self.ready = false;
        }
    }

    /// FUN_140301d80.
    fn show(&mut self, seq: &Sequence, ipic: &Sequence, sp: &mut Sprites, id: u16) {
        if self.own_frame(sp).is_some() || self.alive() {
            if let Some(s) = self.sprite.as_mut() {
                s.dead = true;
            }
            return;
        }
        if let Some(g) = self.frame_groups {
            let f = sp.create(seq, self.prio, 0, g[0], Some(g[2]), None);
            self.frame = Some(f);
        }
        let Some(parent) = self.frame else { return };
        let text_x = seq.groups[sp.get(parent).player.group].text_position_x;
        let text_x = if text_x < 1 { 100 } else { text_x };
        self.sprite = Some(PictureSprite {
            image: id,
            player: Player::new(ipic, self.groups.map_or(SHOW, |g| g[0])),
            parent,
            scale: text_x as f32 / 100.,
            prio: self.prio,
            die_when_done: false,
            dead: false,
        });
    }

    /// The picture's player (a sprite task); it dies after its close group
    /// or with its parent.
    pub fn update(&mut self, ipic: &Sequence, sp: &Sprites) {
        let Some(s) = self.sprite.as_mut() else {
            return;
        };
        if s.dead {
            return;
        }
        if !sp.alive(s.parent) {
            s.dead = true;
            return;
        }
        s.player.update(ipic, 1.);
        if s.die_when_done && ipic.groups[s.player.group].play_count != 0 && s.player.finished() {
            s.dead = true;
        }
    }

    /// The picture's quads at its parent's context, scaled.
    pub fn quads(&self, ipic: &Sequence, sp: &Sprites) -> Option<(u16, u32, Vec<Quad>)> {
        let s = self.sprite.as_ref().filter(|s| !s.dead)?;
        if !sp.alive(s.parent) {
            return None;
        }
        let m = Modifiers {
            scale_x: s.scale,
            scale_y: s.scale,
            ..Modifiers::default()
        };
        Some((
            s.image,
            s.prio,
            ipic.quads(&s.player, sp.context(s.parent), &m),
        ))
    }
}
