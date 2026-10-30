//! Sora's field gauge, translated from the executable's GAUGE_PLAYER object
//! (vtable 0x1405b2100): creation FUN_14017f270 / FUN_14017fe50 /
//! FUN_14017f9b0, per-frame logic FUN_140181270, ring masks FUN_14017f7d0,
//! bars FUN_14017f670, MP FUN_140181af0 and drive FUN_1401816c0. Layout units
//! are the PS2 512x416 2D space; every gauge sprite and mask adds an X origin
//! of 85.
//!
//! The ring fill uses the original's depth trick: depth-only masks are drawn
//! just before each ring, and depth-tested rings only show where their z is
//! greater than the mask's. `Canvas` reproduces that in software.
use data::sprite_sequence::{Modifiers, Player, Quad, Sequence};
use player::follow_camera::wrap;

/// X origin added to every gauge sprite and mask (literal 0x55). Outside
/// 4:3 the HUD anchors right-half elements 85 units further right and
/// left-half ones 85 units further left (0x14017d870, 0x14017d9e0).
pub const X_ORIGIN: f32 = 85.;
/// The 4:3 layout centred in a 16:9 canvas: 512 * (16/9) / (4/3), so layout
/// x = 0 sits CANVAS_PAD units from the canvas's left edge.
pub const CANVAS_WIDTH: f32 = 512. * 4. / 3.;
pub const CANVAS_PAD: f32 = (CANVAS_WIDTH - 512.) / 2.;
pub const CANVAS_HEIGHT: f32 = 416.;
/// Ring angle per HP below 60 (pi/40, 0x14071b004).
const RING_RADIANS_PER_HP: f32 = 0.078_539_8;
/// Bar pixels per HP above 60 (3.0 at 0x1405a9cb8) and bar art width (100,
/// 0x140623be4).
const BAR_PIXELS_PER_HP: f32 = 3.;
const BAR_ART_WIDTH: f32 = 100.;
/// Drive wiper angle per gauge point (0.23 degrees, 0x14071b158).
const DRIVE_RADIANS_PER_POINT: f32 = 0.004_014_26;

/// Per-state tables (normal, low HP, KO): 0x14071afe0, 0x14071afec,
/// 0x14071aff8.
const ROOT_LOOP: [usize; 3] = [137, 132, 137];
const MAX_RING: [usize; 3] = [121, 122, 121];
const MAX_BAR: [usize; 3] = [124, 125, 124];
/// Face group by state (FUN_14017eba0): normal 3, low HP 0, KO 2.
const FACE: [usize; 3] = [3, 0, 2];
/// Drive level digits (0x14071b108).
const DRIVE_DIGITS: [usize; 10] = [28, 29, 30, 31, 32, 33, 34, 35, 36, 37];
/// Form mode (hud-01 §6.1): frame intro 50, loop 51, fill 49, digits
/// 54–63.
const FORM_DIGITS: [usize; 10] = [54, 55, 56, 57, 58, 59, 60, 61, 62, 63];
/// Form-mode wiper: 23° per bar's share of time (0x1405b2664, π/180 at
/// 0x1405b15b4).
const FORM_DEGREES: f32 = 23.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Id {
    Root,
    MaxRing,
    ShadowRing,
    HpRing,
    MaxBar,
    ShadowBar,
    HpBar,
    EndCap,
    Face,
    MpCap,
    MpBackground,
    MpFill,
    MpEndCap,
    /// The MP-charge streak (0x568, group 27), shown while charging.
    MpStreak,
    DriveFrame,
    DriveFill,
    DriveDigit,
    Max,
    /// The ring burst (group 0x82) on a Drive or revert, and the revert's
    /// return flash (group 0x35) on the drive frame (FUN_140180900,
    /// FUN_140180b00).
    Burst,
    Return,
}

struct Element {
    id: Id,
    /// Sheet index: 0 gauge, 1 face.
    sheet: usize,
    player: Player,
    queued: Option<usize>,
    parent: Option<usize>,
    offset: [f32; 2],
    /// Depth tested against masks (GREATER) when non-zero.
    z: i32,
    hidden: bool,
    modifiers: Modifiers,
    /// This frame's child context and base, filled while drawing.
    context: [f32; 2],
    base: [f32; 2],
}

/// Depth-only mask primitives. Quads rotate clockwise about their pivot.
#[derive(Clone, Copy, Debug, PartialEq)]
enum MaskKind {
    /// 100x100 quad centred on the ring.
    Clear,
    /// 64x64 quad with its bottom-right corner on the ring centre, plus a
    /// rectangle chosen by quadrant.
    Ring,
    /// 50x120 quad with its bottom-right corner on the pivot.
    Sector,
}

struct Mask {
    kind: MaskKind,
    /// Element whose context and base place the mask.
    anchor: usize,
    relative: [f32; 2],
    angle: f32,
    rect: Option<[f32; 4]>,
    z: i32,
    hidden: bool,
}

enum Node {
    Sprite(usize),
    Mask(usize),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GaugeStats {
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
    pub max_mp: i32,
    /// Drive gauge mode (0 none, 1 normal), partial 0..=100, full bars and
    /// maximum bars.
    pub drive_mode: u8,
    pub drive: u8,
    pub drive_level: u8,
    pub drive_max: u8,
    /// MP Charge: remaining and total (slot +0x1bc / +0x1c0); None when MP
    /// is not charging.
    pub mp_charge: Option<(f32, f32)>,
    /// Form (or summon) bars, time and maximum time (slot +0x1b3, +0x1b4,
    /// +0x1b8).
    pub form_bars: u8,
    pub form_time: f32,
    pub form_max_time: f32,
}

pub struct Gauge {
    elements: Vec<Element>,
    masks: Vec<Mask>,
    order: Vec<Node>,
    state: usize,
    last_hp: i32,
    shadow_hp: i32,
    last_level: u8,
    /// The drive mode last shown (+0x45d0) and the form digit.
    last_mode: u8,
    last_form_digit: Option<usize>,
    max_shown: bool,
    /// MP Charge is showing (the byte at +0x29 of FUN_140181af0).
    charging: bool,
}

/// A drawable this frame: sprite quads (depth tested when z != 0) or
/// depth-only polygons.
pub enum Draw {
    Quads {
        quads: Vec<Quad>,
        z: i32,
        sheet: usize,
    },
    Mask {
        polygons: Vec<[[f32; 2]; 4]>,
        z: i32,
    },
}

impl Gauge {
    /// `sheets[0]` is the gauge sequence; `sheets[1]`, when present, the
    /// character's face sequence.
    pub fn new(sheets: &[&Sequence], stats: &GaugeStats) -> Self {
        let seq = sheets[0];
        let mut g = Gauge {
            elements: Vec::new(),
            masks: Vec::new(),
            order: Vec::new(),
            state: Self::state_of(stats),
            last_hp: stats.hp,
            shadow_hp: stats.hp,
            last_level: stats.drive_level,
            last_mode: stats.drive_mode,
            last_form_digit: None,
            max_shown: false,
            charging: false,
        };
        let root = g.sprite(seq, Id::Root, 136, Some(ROOT_LOOP[g.state]), None, 0);
        g.mask(MaskKind::Clear, root, 0);
        g.mask(MaskKind::Ring, root, 200);
        g.sprite(seq, Id::MaxRing, MAX_RING[g.state], None, Some(root), 100);
        g.mask(MaskKind::Ring, root, 600);
        g.sprite(seq, Id::ShadowRing, 121, None, Some(root), 500);
        g.mask(MaskKind::Ring, root, 2000);
        g.sprite(seq, Id::HpRing, 128, None, Some(root), 1000);
        g.sprite(seq, Id::MaxBar, MAX_BAR[g.state], None, Some(root), 0);
        g.sprite(seq, Id::ShadowBar, 127, None, Some(root), 0);
        g.sprite(seq, Id::HpBar, 129, None, Some(root), 0);
        g.sprite(seq, Id::EndCap, 123, None, Some(root), 0);
        if let Some(face) = sheets.get(1) {
            let i = g.sprite(face, Id::Face, FACE[g.state], None, Some(root), 0);
            g.elements[i].sheet = 1;
        }
        let cap = g.sprite(seq, Id::MpCap, 116, Some(117), Some(root), 0);
        g.sprite(seq, Id::MpBackground, 119, None, Some(cap), 0);
        g.sprite(seq, Id::MpFill, 115, None, Some(cap), 0);
        g.sprite(seq, Id::MpEndCap, 120, None, Some(cap), 0);
        // Created hidden; priority 7005.
        let streak = g.sprite(seq, Id::MpStreak, 27, None, Some(root), 0);
        g.elements[streak].hidden = true;
        let frame = g.sprite(seq, Id::DriveFrame, 21, Some(23), Some(root), 0);
        let sector = g.mask(MaskKind::Sector, frame, 6000);
        g.masks[sector].relative = [50., 100.];
        g.sprite(seq, Id::DriveFill, 25, None, Some(frame), 5000);
        let digit = DRIVE_DIGITS[stats.drive_level.min(9) as usize];
        let digit = g.sprite(seq, Id::DriveDigit, digit, None, Some(frame), 0);
        g.elements[digit].offset = [52., -2.];
        // Priority 7010: drawn after the 7000 elements.
        let max = g.sprite(seq, Id::Max, 20, None, Some(frame), 0);
        g.elements[max].hidden = true;
        // Pool sprites, created hidden (prio 7000, inserted after the
        // gauge's own: drawn on top).
        let burst = g.sprite(seq, Id::Burst, 130, None, Some(root), 0);
        g.elements[burst].hidden = true;
        let ret = g.sprite(seq, Id::Return, 53, None, Some(frame), 0);
        g.elements[ret].hidden = true;
        g
    }

    fn sprite(
        &mut self,
        seq: &Sequence,
        id: Id,
        group: usize,
        queued: Option<usize>,
        parent: Option<usize>,
        z: i32,
    ) -> usize {
        self.elements.push(Element {
            id,
            sheet: 0,
            player: Player::new(seq, group),
            queued,
            parent,
            offset: [0.; 2],
            z,
            hidden: false,
            modifiers: Modifiers::default(),
            context: [0.; 2],
            base: [0.; 2],
        });
        self.order.push(Node::Sprite(self.elements.len() - 1));
        self.elements.len() - 1
    }

    fn mask(&mut self, kind: MaskKind, anchor: usize, z: i32) -> usize {
        self.masks.push(Mask {
            kind,
            anchor,
            relative: [0.; 2],
            angle: 0.,
            rect: None,
            z,
            hidden: kind == MaskKind::Ring,
        });
        self.order.push(Node::Mask(self.masks.len() - 1));
        self.masks.len() - 1
    }

    fn find(&self, id: Id) -> usize {
        self.elements.iter().position(|e| e.id == id).unwrap()
    }

    fn set_group(&mut self, seq: &Sequence, id: Id, group: usize) {
        let i = self.find(id);
        self.elements[i].player = Player::new(seq, group);
        self.elements[i].queued = None;
        self.elements[i].hidden = false;
    }

    fn set_face(&mut self, sheets: &[&Sequence], group: usize) {
        let Some(face) = sheets.get(1) else { return };
        let i = self.find(Id::Face);
        self.elements[i].player = Player::new(face, group);
        self.elements[i].hidden = false;
    }

    /// Low HP is HP <= MaxHP / 4 with integer division (0x1403c1d30).
    fn state_of(stats: &GaugeStats) -> usize {
        (stats.hp <= stats.max_hp / 4) as usize
    }

    /// Ring mask for value v (FUN_14017f7d0): hidden at 60 and above.
    fn ring_mask(&mut self, mask: usize, v: i32, end_cap: bool) {
        self.masks[mask].hidden = true;
        if v >= 60 {
            return;
        }
        let theta = wrap(v as f32 * RING_RADIANS_PER_HP);
        let m = &mut self.masks[mask];
        m.angle = theta;
        m.rect = Some(if v < 20 {
            [0., -50., 64., 100.]
        } else if v < 40 {
            [0., 0., 64., 50.]
        } else {
            [0., 50., 0., 0.]
        });
        m.hidden = false;
        if end_cap {
            let cap = self.find(Id::EndCap);
            self.elements[cap].modifiers.rotation = -theta;
            self.elements[cap].offset = [0., 0.];
        }
    }

    /// HP-family bar for value v (FUN_14017f670): grows left 3 px per HP
    /// above 60.
    fn bar(&mut self, id: Id, v: i32, end_cap: bool) {
        let i = self.find(id);
        self.elements[i].hidden = true;
        if v < 60 {
            return;
        }
        let len = (v - 60) as f32 * BAR_PIXELS_PER_HP;
        self.elements[i].modifiers.scale_x = len / BAR_ART_WIDTH;
        self.elements[i].hidden = false;
        if end_cap {
            let cap = self.find(Id::EndCap);
            self.elements[cap].modifiers.rotation = wrap(-4.712_389);
            self.elements[cap].offset = [-((len as i32) as f32), 0.];
        }
    }

    /// One 60 Hz frame of the gauge's own logic (FUN_140181270), then each
    /// element's player in creation order (FUN_14016b3c0).
    pub fn update(&mut self, sheets: &[&Sequence], stats: &GaugeStats) {
        let seq = sheets[0];
        let state = Self::state_of(stats);
        if state != self.state {
            self.state = state;
            let root = self.find(Id::Root);
            if self.elements[root].queued.is_some() {
                self.elements[root].queued = Some(ROOT_LOOP[state]);
            } else {
                self.set_group(seq, Id::Root, ROOT_LOOP[state]);
            }
            self.set_group(seq, Id::MaxRing, MAX_RING[state]);
            self.set_group(seq, Id::MaxBar, MAX_BAR[state]);
            self.set_face(sheets, FACE[state]);
        }
        if stats.hp != self.last_hp {
            self.shadow_hp = self.last_hp.max(stats.hp);
            self.last_hp = stats.hp;
            self.set_group(seq, Id::ShadowRing, 126);
            self.set_group(seq, Id::ShadowBar, 127);
        }
        let rings: Vec<usize> = (0..self.masks.len())
            .filter(|&i| self.masks[i].kind == MaskKind::Ring)
            .collect();
        self.ring_mask(rings[0], stats.max_hp, true);
        self.bar(Id::MaxBar, stats.max_hp, true);
        self.ring_mask(rings[1], self.shadow_hp, false);
        self.bar(Id::ShadowBar, self.shadow_hp, false);
        self.ring_mask(rings[2], stats.hp, false);
        self.bar(Id::HpBar, stats.hp, false);
        // MP Charge (FUN_140181af0): entering swaps the MP groups to
        // 110 / 113 / 109 / 114 (queued 111) and the Drive frame loop to 26,
        // and shows the streak; leaving restores 116 / 119 / 115 / 120
        // (queued 117) and 23.
        let charging = stats.mp_charge.is_some();
        if charging != self.charging {
            self.charging = charging;
            let (cap, bg, fill, end, queued, frame) = if charging {
                (110, 113, 109, 114, 111, 26)
            } else {
                (116, 119, 115, 120, 117, 23)
            };
            self.set_group(seq, Id::MpCap, cap);
            let i = self.find(Id::MpCap);
            self.elements[i].queued = Some(queued);
            self.set_group(seq, Id::MpBackground, bg);
            self.set_group(seq, Id::MpFill, fill);
            self.set_group(seq, Id::MpEndCap, end);
            if stats.drive_mode <= 1 {
                self.set_group(seq, Id::DriveFrame, frame);
            }
            let streak = self.find(Id::MpStreak);
            if charging && !matches!(stats.drive_mode, 2..=4) {
                self.set_group(seq, Id::MpStreak, 27);
            } else {
                self.elements[streak].hidden = true;
            }
        }
        let bg = self.find(Id::MpBackground);
        let fill = self.find(Id::MpFill);
        let end = self.find(Id::MpEndCap);
        match stats.mp_charge {
            Some((remaining, total)) => {
                // Charging: background 1.0, end cap at -100, fill by the
                // remaining charge (0x1403c20e0).
                self.elements[bg].modifiers.scale_x = 1.;
                self.elements[end].offset = [-100., 0.];
                self.elements[fill].modifiers.scale_x =
                    if total > 0. { remaining / total } else { 0. };
            }
            None => {
                // Not charging: 1 MP = 1 px (0.1 at 0x1406239f8).
                self.elements[bg].modifiers.scale_x = (stats.max_mp as f32 + 0.1) / 100.;
                self.elements[fill].modifiers.scale_x = (stats.mp as f32 + 0.1) / 100.;
                self.elements[end].offset = [-(((stats.max_mp as f32 + 0.1) as i32) as f32), 0.];
            }
        }
        // Mode changes (vs +0x45d0): entering a form swaps the frame to
        // 50 (looping 51) and the fill to 49; leaving it returns to the
        // Drive groups (FUN_140180b00).
        let form = matches!(stats.drive_mode, 2 | 3);
        if stats.drive_mode != self.last_mode {
            let was_form = matches!(self.last_mode, 2 | 3);
            self.last_mode = stats.drive_mode;
            if form && !was_form {
                // FUN_140180900: the ring burst, the FORM frame (0x33)
                // and bar (0x31).
                self.set_group(seq, Id::Burst, 130);
                self.set_group(seq, Id::DriveFrame, 51);
                self.set_group(seq, Id::DriveFill, 49);
                self.last_form_digit = None;
            } else if !form && was_form {
                // FUN_140180b00: straight to the loop group, the burst and
                // the return flash.
                self.set_group(seq, Id::Burst, 130);
                self.set_group(seq, Id::Return, 53);
                self.set_group(seq, Id::DriveFrame, if self.charging { 26 } else { 23 });
                self.set_group(seq, Id::DriveFill, 25);
                let digit = DRIVE_DIGITS[stats.drive_level.min(9) as usize];
                self.set_group(seq, Id::DriveDigit, digit);
                self.last_level = stats.drive_level;
            }
        }
        let sector = (0..self.masks.len())
            .find(|&i| self.masks[i].kind == MaskKind::Sector)
            .unwrap();
        self.masks[sector].hidden = stats.drive_mode == 0;
        if form {
            // Form mode (hud-01 §6.3): the wiper shows the current bar's
            // share of time; the digit is the bar, MAX is hidden.
            let bars = stats.form_bars.max(1) as i32;
            let per = ((stats.form_max_time / bars as f32) as i32).max(1);
            let t = stats.form_time as i32;
            let rem = if t / per == bars { per } else { t % per };
            self.masks[sector].angle =
                wrap(FORM_DEGREES / per as f32 * std::f32::consts::PI / 180. * rem as f32);
            let digit = FORM_DIGITS[(t / per).min(bars - 1).clamp(0, 9) as usize];
            if self.last_form_digit != Some(digit) {
                self.last_form_digit = Some(digit);
                self.set_group(seq, Id::DriveDigit, digit);
            }
        } else {
            // Drive, normal mode: a full gauge (100) uses 200 (0x140585efc).
            let d = if stats.drive == 100 {
                200.
            } else {
                stats.drive as f32
            };
            self.masks[sector].angle = wrap(d * DRIVE_RADIANS_PER_POINT);
            if stats.drive_level != self.last_level {
                self.last_level = stats.drive_level;
                let digit = DRIVE_DIGITS[stats.drive_level.min(9) as usize];
                self.set_group(seq, Id::DriveDigit, digit);
            }
        }
        let max = self.find(Id::Max);
        if form || stats.drive_level < stats.drive_max {
            self.elements[max].hidden = true;
            self.max_shown = false;
        } else if !self.max_shown {
            self.max_shown = true;
            self.set_group(seq, Id::Max, 20);
        }
        if stats.drive_mode == 0 {
            for id in [Id::DriveFrame, Id::DriveFill, Id::DriveDigit] {
                let i = self.find(id);
                self.elements[i].hidden = true;
            }
        }
        // A finished one-shot group switches to its queued group and
        // advances once, or the sprite hides.
        advance(&mut self.elements, sheets);
    }

    /// This frame's drawables in draw order, in canvas layout units (X origin
    /// applied). A child sits at its parent's child context + offset + base.
    pub fn draw(&mut self, sheets: &[&Sequence]) -> Vec<Draw> {
        draw_nodes(&self.order, &mut self.elements, &self.masks, sheets)
    }
}

/// A finished one-shot group switches to its queued group and advances once,
/// or the sprite hides.
fn advance(elements: &mut [Element], sheets: &[&Sequence]) {
    for e in elements {
        let seq = sheets[e.sheet];
        e.player.update(seq, 1.);
        if seq.groups[e.player.group].play_count != 0 && e.player.finished() {
            if let Some(next) = e.queued.take() {
                e.player = Player::new(seq, next);
                e.player.update(seq, 1.);
            } else {
                e.hidden = true;
            }
        }
    }
}

/// The drawables of a gauge's nodes in creation order.
fn draw_nodes(
    order: &[Node],
    elements: &mut [Element],
    masks: &[Mask],
    sheets: &[&Sequence],
) -> Vec<Draw> {
    let mut out = Vec::new();
    for node in order {
        match *node {
            Node::Sprite(i) => {
                let base = elements[i].parent.map_or([0.; 2], |p| {
                    let p = &elements[p];
                    [
                        p.context[0] + p.offset[0] + p.base[0],
                        p.context[1] + p.offset[1] + p.base[1],
                    ]
                });
                let e = &mut elements[i];
                let seq = sheets[e.sheet];
                e.base = base;
                e.context = seq
                    .child_context(&e.player, &e.modifiers)
                    .unwrap_or([0.; 2]);
                if e.hidden {
                    continue;
                }
                let origin = [X_ORIGIN + e.offset[0] + base[0], e.offset[1] + base[1]];
                out.push(Draw::Quads {
                    quads: seq.quads(&e.player, origin, &e.modifiers),
                    z: e.z,
                    sheet: e.sheet,
                });
            }
            Node::Mask(m) => {
                let mask = &masks[m];
                if mask.hidden {
                    continue;
                }
                let a = &elements[mask.anchor];
                // The anchor's own offset (FUN_14016b0c0) places the mask too.
                let pivot = [
                    X_ORIGIN + a.context[0] + a.base[0] + a.offset[0] + mask.relative[0],
                    a.context[1] + a.base[1] + a.offset[1] + mask.relative[1],
                ];
                let (s, c) = mask.angle.sin_cos();
                // Clockwise on a Y-down screen.
                let place = |x: f32, y: f32| [pivot[0] + x * c - y * s, pivot[1] + x * s + y * c];
                let quad = |[x0, y0, x1, y1]: [f32; 4]| {
                    [place(x0, y0), place(x1, y0), place(x0, y1), place(x1, y1)]
                };
                let mut polygons = vec![match mask.kind {
                    MaskKind::Clear => quad([-50., -50., 50., 50.]),
                    MaskKind::Ring => quad([-64., -64., 0., 0.]),
                    MaskKind::Sector => quad([-50., -120., 0., 0.]),
                }];
                if let Some([dx, dy, w, h]) = mask.rect
                    && w > 0.
                    && h > 0.
                {
                    let (x0, y0) = (pivot[0] + dx, pivot[1] + dy);
                    polygons.push([[x0, y0], [x0 + w, y0], [x0, y0 + h], [x0 + w, y0 + h]]);
                }
                out.push(Draw::Mask {
                    polygons,
                    z: mask.z,
                });
            }
        }
    }
    out
}

/// Donald's and Goofy's field gauge, the executable's GAUGE_FRIEND object
/// (vtable 0x1405b2158): created by FUN_14016c3a0 / FUN_140181d20 from the
/// same gauge sequence as Sora's, updated by FUN_140182730 and the event
/// handlers FUN_140182530 / FUN_140182360 (events posted by FUN_1403d60c0
/// for a hit and FUN_1403d2eb0 for a heal). A ring of two halves: the
/// shadow (group 0x40) and HP (0x41) masked by the left half's wipe, the MP
/// ring (0x42) by the right half's; the face sits in the middle.
///
/// The Goofy gauge's slide when Donald is absent (FUN_140182730, groups
/// 0x49 / 0x4a) is not built: the room always has both members.
const FRIEND_INTRO: usize = 0x47;
/// Loop group by state (FUN_14017ec50): low HP 0x43, else 0x48.
const FRIEND_LOOP: [usize; 3] = [0x48, 0x43, 0x48];
/// The flash after a hit of event 1, 2 and 3 (FUN_1401823d0, 1401823c0,
/// 1401823b0).
const FRIEND_FLASH: [usize; 3] = [0x46, 0x45, 0x44];
const FRIEND_SHADOW: usize = 0x40;
const FRIEND_HP: usize = 0x41;
const FRIEND_MP: usize = 0x42;
/// The face while a hit reacts (FUN_14017ebe0 plays group 1).
const FRIEND_FACE_HIT: usize = 1;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FriendStats {
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
    pub max_mp: i32,
    /// The HUD event of this tick: 0 none, 1 hit, 2 big hit, 3 blow or KO
    /// hit, 4 heal.
    pub event: u8,
}

pub struct FriendGauge {
    elements: Vec<Element>,
    masks: Vec<Mask>,
    order: Vec<Node>,
    /// Normal 0, low HP 1, KO 2 (obj+0x20).
    state: usize,
    /// The HP the last two events saw (obj+0x30 and +0x2c): the shadow ring
    /// shows the older one.
    current: i32,
    shadow: i32,
    face_sheet: Option<usize>,
}

impl FriendGauge {
    /// `sheets[0]` is the gauge sequence; `face` is the sheet index of the
    /// member's own face sequence; `slot` 0 is Donald's, 1 Goofy's.
    pub fn new(
        sheets: &[&Sequence],
        face: Option<usize>,
        slot: usize,
        stats: &FriendStats,
    ) -> Self {
        let seq = sheets[0];
        let mut g = FriendGauge {
            elements: Vec::new(),
            masks: Vec::new(),
            order: Vec::new(),
            state: Self::state_of(stats),
            current: stats.hp,
            shadow: stats.hp,
            face_sheet: face,
        };
        // FUN_140181d20: the root plays the intro group and queues the loop;
        // the second gauge sits one group pitch (+0x20 of the group) lower.
        let root = g.sprite(
            seq,
            Id::Root,
            FRIEND_INTRO,
            Some(FRIEND_LOOP[g.state]),
            None,
            0,
        );
        let pitch = seq
            .groups
            .get(FRIEND_INTRO)
            .map_or(0, |p| p.text_position_x);
        g.elements[root].offset = [0., (pitch * slot as i32) as f32];
        g.mask(MaskKind::Clear, root, 0);
        g.mask(MaskKind::Ring, root, 200);
        g.sprite(seq, Id::ShadowRing, FRIEND_SHADOW, None, Some(root), 100);
        g.mask(MaskKind::Ring, root, 2000);
        g.sprite(seq, Id::HpRing, FRIEND_HP, None, Some(root), 1000);
        g.mask(MaskKind::Clear, root, 0);
        g.mask(MaskKind::Ring, root, 20000);
        g.sprite(seq, Id::MpFill, FRIEND_MP, None, Some(root), 10000);
        if let Some(sheet) = face {
            let i = g.sprite(sheets[sheet], Id::Face, FACE[g.state], None, Some(root), 0);
            g.elements[i].sheet = sheet;
        }
        for m in &mut g.masks {
            m.hidden = false;
        }
        g
    }

    fn sprite(
        &mut self,
        seq: &Sequence,
        id: Id,
        group: usize,
        queued: Option<usize>,
        parent: Option<usize>,
        z: i32,
    ) -> usize {
        self.elements.push(Element {
            id,
            sheet: 0,
            player: Player::new(seq, group),
            queued,
            parent,
            offset: [0.; 2],
            z,
            hidden: false,
            modifiers: Modifiers::default(),
            context: [0.; 2],
            base: [0.; 2],
        });
        self.order.push(Node::Sprite(self.elements.len() - 1));
        self.elements.len() - 1
    }

    fn mask(&mut self, kind: MaskKind, anchor: usize, z: i32) {
        self.masks.push(Mask {
            kind,
            anchor,
            relative: [0.; 2],
            angle: 0.,
            rect: None,
            z,
            hidden: false,
        });
        self.order.push(Node::Mask(self.masks.len() - 1));
    }

    fn find(&self, id: Id) -> usize {
        self.elements.iter().position(|e| e.id == id).unwrap()
    }

    /// FUN_14017ec50: dead 2, HP at most a quarter of the maximum 1.
    fn state_of(stats: &FriendStats) -> usize {
        if stats.hp <= 0 {
            2
        } else {
            (stats.hp <= stats.max_hp / 4) as usize
        }
    }

    /// FUN_140182180: the left half's wipe for the HP share `r` (mask 1 for
    /// the shadow's share, mask 2 for the current one).
    fn hp_wipe(&mut self, mask: usize, r: f32) {
        let m = &mut self.masks[mask];
        m.angle = wrap(r * std::f32::consts::PI - std::f32::consts::FRAC_PI_2);
        m.rect = if r == 0. {
            Some([-64., -62., 64., 68.])
        } else if r >= 0.5 {
            None
        } else {
            Some([-64., -64., 64., 64.])
        };
    }

    /// The tail of FUN_140182730: the right half's wipe for the MP share.
    fn mp_wipe(&mut self, mask: usize, r: f32) {
        let m = &mut self.masks[mask];
        m.angle = wrap((1. - r) * std::f32::consts::PI);
        m.rect = if r == 0. {
            Some([0., -62., 64., 68.])
        } else if r >= 0.5 {
            // The unset 1 x 1 rectangle (the executable's `uVar5 = 1`).
            Some([-64., 0., 1., 1.])
        } else {
            Some([0., -64., 64., 64.])
        };
    }

    fn set_root(&mut self, seq: &Sequence, group: usize, queued: Option<usize>) {
        let i = self.find(Id::Root);
        self.elements[i].player = Player::new(seq, group);
        self.elements[i].queued = queued;
        self.elements[i].hidden = false;
    }

    fn set_face(&mut self, sheets: &[&Sequence], group: usize, queued: Option<usize>) {
        let Some(sheet) = self.face_sheet else { return };
        let i = self.find(Id::Face);
        self.elements[i].player = Player::new(sheets[sheet], group);
        self.elements[i].queued = queued;
        self.elements[i].hidden = false;
    }

    /// One 60 Hz frame (FUN_140182730): the state, the HUD event, the three
    /// wipes, then every element's player.
    pub fn update(&mut self, sheets: &[&Sequence], stats: &FriendStats) {
        let seq = sheets[0];
        // FUN_14017ee60: the state is read before the events.
        self.state = Self::state_of(stats);
        match stats.event {
            // FUN_140182530: the shadow keeps the HP of the event before,
            // the shadow ring restarts, the root flashes and queues its
            // loop, the face reacts.
            1..=3 => {
                self.shadow = self.current;
                self.current = stats.hp;
                let i = self.find(Id::ShadowRing);
                self.elements[i].player = Player::new(seq, FRIEND_SHADOW);
                self.elements[i].queued = None;
                self.elements[i].hidden = false;
                self.set_root(
                    seq,
                    FRIEND_FLASH[stats.event as usize - 1],
                    Some(FRIEND_LOOP[self.state]),
                );
                self.set_face(sheets, FRIEND_FACE_HIT, Some(FACE[self.state]));
            }
            // FUN_140182360: a heal makes the shadow the current HP.
            4 => {
                self.shadow = stats.hp;
                self.current = stats.hp;
                self.set_root(seq, FRIEND_LOOP[self.state], None);
                self.set_face(sheets, FACE[self.state], None);
            }
            _ => {}
        }
        let share = |v: i32, max: i32| {
            if max > 0 { v as f32 / max as f32 } else { 0. }
        };
        self.hp_wipe(1, share(self.shadow, stats.max_hp));
        self.hp_wipe(2, share(stats.hp, stats.max_hp));
        self.mp_wipe(4, share(stats.mp, stats.max_mp));
        advance(&mut self.elements, sheets);
    }

    /// This frame's drawables in creation order.
    pub fn draw(&mut self, sheets: &[&Sequence]) -> Vec<Draw> {
        draw_nodes(&self.order, &mut self.elements, &self.masks, sheets)
    }
}

/// Software rasterizer for the gauge: a straight-alpha RGBA canvas plus a
/// depth buffer emulating the GS depth trick (masks write depth only;
/// depth-tested sprites draw where their z is greater).
///
/// The canvas is kept in `TILE`-pixel squares. Each frame hashes, per tile,
/// every triangle that reaches it, in draw order; a tile's pixels depend on
/// nothing else, so a tile whose hash is the last frame's already holds this
/// frame's pixels and is left alone. Only changed tiles are cleared, drawn
/// and resolved.
pub struct Canvas {
    pub width: usize,
    pub height: usize,
    /// Canvas pixels per layout unit.
    pub scale: f32,
    pub rgba: Vec<u8>,
    color: Vec<[f32; 4]>,
    depth: Vec<i32>,
    tiles_x: usize,
    /// Per tile: this frame's and the last frame's hash (0: nothing drawn).
    hash: Vec<u64>,
    last_hash: Vec<u64>,
    /// Tiles this frame redrew.
    redraw: Vec<bool>,
    /// Threads for a large frame.
    pub threads: usize,
}

/// Side of the canvas tiles, in pixels.
const TILE: usize = 16;

pub struct Texture<'a> {
    pub rgba: &'a [u8],
    pub size: [usize; 2],
    /// Texture texels per sequence texel.
    pub texel_scale: f32,
}

/// `b as f32 / 255.` for every byte, computed once.
const UNIT: [f32; 256] = {
    let mut t = [0.; 256];
    let mut i = 0;
    while i < 256 {
        t[i] = i as f32 / 255.;
        i += 1;
    }
    t
};

impl Texture<'_> {
    /// A fingerprint for the tile hashes: the pixels' address, length,
    /// size and scale, and 64 texels spread over them (a sheet replaced by
    /// one at the same address shows as a change; `Canvas::forget` covers
    /// the rest).
    fn fingerprint(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::hash::DefaultHasher::new();
        (self.rgba.as_ptr() as usize, self.rgba.len(), self.size).hash(&mut h);
        self.texel_scale.to_bits().hash(&mut h);
        let texels = self.rgba.len() / 4;
        for k in 0..64.min(texels) {
            let o = k * texels / 64 * 4;
            self.rgba[o..o + 4].hash(&mut h);
        }
        h.finish()
    }

    /// Bilinear sample in texels, clamped to the edge.
    fn sample(&self, u: f32, v: f32) -> [f32; 4] {
        let [tw, th] = self.size;
        let (u, v) = (u - 0.5, v - 0.5);
        let (x0, y0) = (u.floor(), v.floor());
        let (fx, fy) = (u - x0, v - y0);
        let at = |x: f32, y: f32| {
            let x = (x.max(0.) as usize).min(tw - 1);
            let y = (y.max(0.) as usize).min(th - 1);
            let o = (y * tw + x) * 4;
            let p: &[u8; 4] = self.rgba[o..o + 4].try_into().unwrap();
            p.map(|c| UNIT[c as usize])
        };
        let (a, b, c, d) = (
            at(x0, y0),
            at(x0 + 1., y0),
            at(x0, y0 + 1.),
            at(x0 + 1., y0 + 1.),
        );
        std::array::from_fn(|i| {
            let top = a[i] + (b[i] - a[i]) * fx;
            let bottom = c[i] + (d[i] - c[i]) * fx;
            top + (bottom - top) * fy
        })
    }
}

impl Canvas {
    pub fn new(scale: f32) -> Self {
        let width = (CANVAS_WIDTH * scale).ceil() as usize;
        let height = (CANVAS_HEIGHT * scale).ceil() as usize;
        let tiles_x = width.div_ceil(TILE);
        let tiles = tiles_x * height.div_ceil(TILE);
        Canvas {
            width,
            height,
            scale,
            rgba: vec![0; width * height * 4],
            color: vec![[0.; 4]; width * height],
            depth: vec![0; width * height],
            tiles_x,
            hash: vec![0; tiles],
            last_hash: vec![0; tiles],
            redraw: vec![false; tiles],
            threads: std::thread::available_parallelism().map_or(1, |n| n.get().min(8)),
        }
    }

    /// Redraw every tile next frame (after replacing a texture in place).
    pub fn forget(&mut self) {
        // Tile hashes are 0 or odd, so 2 matches none.
        self.last_hash.fill(2);
    }

    /// A tile's pixel rectangle.
    fn tile_rect(&self, t: usize) -> [usize; 4] {
        let (x0, y0) = ((t % self.tiles_x) * TILE, (t / self.tiles_x) * TILE);
        [
            x0,
            y0,
            (x0 + TILE).min(self.width),
            (y0 + TILE).min(self.height),
        ]
    }

    /// Draws this frame; `rgba` then holds straight-alpha pixels. Returns
    /// whether any pixel may have changed. Large frames (the full-screen
    /// iris and fade) draw on several threads, rows in turn; every pixel
    /// still sees the draws in order, so the result does not depend on the
    /// split.
    pub fn render(&mut self, draws: &[Draw], textures: &[Texture]) -> bool {
        use std::hash::{Hash, Hasher};
        let hashing = bevy::log::info_span!("hud_hash").entered();
        let k = self.scale;
        let prints: Vec<u64> = textures.iter().map(Texture::fingerprint).collect();
        let mut tris = Vec::new();
        for draw in draws {
            match draw {
                Draw::Mask { polygons, z } => {
                    for q in polygons {
                        let q = q.map(|[x, y]| [(x + CANVAS_PAD) * k, y * k]);
                        for p in [[q[0], q[1], q[2]], [q[1], q[3], q[2]]] {
                            tris.extend(Tri::new(p, Fill::Mask(*z)));
                        }
                    }
                }
                Draw::Quads { quads, z, sheet } => {
                    let texture = &textures[*sheet];
                    for quad in quads {
                        let q = quad.corners.map(|[x, y]| [(x + CANVAS_PAD) * k, y * k]);
                        let [u0, v0, u1, v1] = quad.uv.map(|v| v as f32 * texture.texel_scale);
                        let uv = [[u0, v0], [u1, v0], [u0, v1], [u1, v1]];
                        for t in [[0usize, 1, 2], [1, 3, 2]] {
                            let fill = Fill::Sprite {
                                quad,
                                t,
                                uv,
                                z: *z,
                                texture,
                            };
                            tris.extend(Tri::new(t.map(|i| q[i]), fill).map(|mut tri| {
                                tri.sheet = *sheet;
                                tri
                            }));
                        }
                    }
                }
            }
        }
        // Pixel bounds clipped to the canvas, then each tile's hash of the
        // triangles over it.
        let (width, height) = (self.width, self.height);
        tris.retain_mut(|tri| match bounds(&tri.p, width, height) {
            Some(b) => {
                tri.b = b;
                true
            }
            None => false,
        });
        self.hash.fill(0);
        for tri in &tris {
            let mut h = std::hash::DefaultHasher::new();
            tri.p.map(|v| v.map(f32::to_bits)).hash(&mut h);
            match &tri.fill {
                Fill::Mask(z) => (0u8, *z).hash(&mut h),
                Fill::Sprite { quad, t, uv, z, .. } => {
                    (1u8, *z, *t, quad.uv, quad.blend, prints[tri.sheet]).hash(&mut h);
                    quad.colors.map(|c| c.map(f32::to_bits)).hash(&mut h);
                    uv.map(|v| v.map(f32::to_bits)).hash(&mut h);
                }
            }
            let one = h.finish();
            let b = tri.b;
            for ty in b[1] / TILE..b[3].div_ceil(TILE) {
                let row = ty * self.tiles_x;
                for t in &mut self.hash[row + b[0] / TILE..row + b[2].div_ceil(TILE)] {
                    *t = ((*t ^ one).wrapping_mul(0x9e37_79b9_7f4a_7c15)).rotate_left(23) | 1;
                }
            }
        }
        let mut rows = [usize::MAX, 0];
        let mut any = false;
        for t in 0..self.hash.len() {
            let r = self.hash[t] != self.last_hash[t];
            self.redraw[t] = r;
            if r {
                any = true;
                let [x0, y0, x1, y1] = self.tile_rect(t);
                rows = [rows[0].min(y0), rows[1].max(y1)];
                for y in y0..y1 {
                    let row = y * width;
                    self.color[row + x0..row + x1].fill([0.; 4]);
                    self.depth[row + x0..row + x1].fill(0);
                    self.rgba[(row + x0) * 4..(row + x1) * 4].fill(0);
                }
            }
        }
        std::mem::swap(&mut self.hash, &mut self.last_hash);
        drop(hashing);
        if !any {
            return false;
        }
        let _raster = bevy::log::info_span!("hud_raster").entered();
        // Only triangles over a redrawn tile draw; rows go to the threads
        // in turn, so drawing that sits in a few rows still spreads.
        let (tiles_x, redraw) = (self.tiles_x, &self.redraw);
        tris.retain(|tri| {
            let b = tri.b;
            (b[1] / TILE..b[3].div_ceil(TILE)).any(|ty| {
                redraw[ty * tiles_x + b[0] / TILE..ty * tiles_x + b[2].div_ceil(TILE)]
                    .contains(&true)
            })
        });
        let work: usize = tris
            .iter()
            .map(|t| (t.b[2] - t.b[0]) * (t.b[3] - t.b[1]))
            .sum();
        let threads = if work >= 1 << 16 { self.threads } else { 1 };
        let mut bands: Vec<Band> = (0..threads)
            .map(|k| Band {
                first: rows[0] + k,
                step: threads,
                end: rows[1],
                tiles_x,
                redraw,
                rows: Vec::new(),
            })
            .collect();
        let (y0, y1) = (rows[0] * width, rows[1] * width);
        for (n, ((color, depth), rgba)) in self.color[y0..y1]
            .chunks_mut(width)
            .zip(self.depth[y0..y1].chunks_mut(width))
            .zip(self.rgba[y0 * 4..y1 * 4].chunks_mut(width * 4))
            .enumerate()
        {
            bands[n % threads].rows.push(Row { color, depth, rgba });
        }
        let tris = &tris;
        if threads == 1 {
            for mut band in bands {
                band.draw(tris);
            }
        } else {
            std::thread::scope(|s| {
                for mut band in bands {
                    s.spawn(move || band.draw(tris));
                }
            });
        }
        true
    }

    /// The rectangles (`[x, y, width, height]` in pixels) the last `render`
    /// redrew: each tile row's runs of redrawn tiles, with their RGBA rows.
    pub fn redrawn(&self) -> Vec<([u32; 4], Vec<u8>)> {
        let mut out = Vec::new();
        for (ty, row) in self.redraw.chunks(self.tiles_x).enumerate() {
            let mut tx = 0;
            while tx < row.len() {
                if !row[tx] {
                    tx += 1;
                    continue;
                }
                let start = tx;
                while tx < row.len() && row[tx] {
                    tx += 1;
                }
                let [x0, y0, _, y1] = self.tile_rect(ty * self.tiles_x + start);
                let x1 = (tx * TILE).min(self.width);
                let mut rgba = Vec::with_capacity((x1 - x0) * (y1 - y0) * 4);
                for y in y0..y1 {
                    rgba.extend_from_slice(
                        &self.rgba[(y * self.width + x0) * 4..(y * self.width + x1) * 4],
                    );
                }
                out.push(([x0, y0, x1 - x0, y1 - y0].map(|v| v as u32), rgba));
            }
        }
        out
    }

    /// Copies the tiles the last `render` redrew into `out` (a copy of
    /// `rgba` from before it); the rest of `out` already matches.
    pub fn copy_redrawn(&self, out: &mut [u8]) {
        for t in (0..self.redraw.len()).filter(|&t| self.redraw[t]) {
            let [x0, y0, x1, y1] = self.tile_rect(t);
            for y in y0..y1 {
                let (a, b) = ((y * self.width + x0) * 4, (y * self.width + x1) * 4);
                out[a..b].copy_from_slice(&self.rgba[a..b]);
            }
        }
    }
}

/// Pixel bounds of a triangle clipped to the canvas.
fn bounds(p: &[[f32; 2]; 3], width: usize, height: usize) -> Option<[usize; 4]> {
    let min = |k: usize| p.iter().map(|v| v[k]).fold(f32::MAX, f32::min);
    let max = |k: usize| p.iter().map(|v| v[k]).fold(f32::MIN, f32::max);
    let x0 = min(0).floor().max(0.) as usize;
    let y0 = min(1).floor().max(0.) as usize;
    let x1 = (max(0).ceil().max(0.) as usize).min(width);
    let y1 = (max(1).ceil().max(0.) as usize).min(height);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    Some([x0, y0, x1, y1])
}

/// What a triangle writes: depth only (a mask) or a textured sprite.
enum Fill<'a> {
    Mask(i32),
    Sprite {
        quad: &'a Quad,
        /// The quad corners of this triangle.
        t: [usize; 3],
        uv: [[f32; 2]; 4],
        z: i32,
        texture: &'a Texture<'a>,
    },
}

struct Tri<'a> {
    p: [[f32; 2]; 3],
    area: f32,
    /// Pixel bounds (set once clipped).
    b: [usize; 4],
    /// The texture's index (sprites).
    sheet: usize,
    fill: Fill<'a>,
}

impl<'a> Tri<'a> {
    /// None for a degenerate triangle.
    fn new(p: [[f32; 2]; 3], fill: Fill<'a>) -> Option<Self> {
        let area =
            (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1]);
        (area.abs() >= 1e-6).then_some(Self {
            p,
            area,
            b: [0; 4],
            sheet: 0,
            fill,
        })
    }
}

/// One canvas row of every pixel's colour, depth and resolved bytes.
struct Row<'a> {
    color: &'a mut [[f32; 4]],
    depth: &'a mut [i32],
    rgba: &'a mut [u8],
}

/// One thread's share of the frame: rows `first`, `first + step`, ...
/// before `end`.
struct Band<'a> {
    first: usize,
    step: usize,
    end: usize,
    tiles_x: usize,
    /// The tiles this frame redraws.
    redraw: &'a [bool],
    rows: Vec<Row<'a>>,
}

impl Band<'_> {
    fn draw(&mut self, tris: &[Tri]) {
        for tri in tris {
            self.raster(tri);
        }
        for (n, row) in self.rows.iter_mut().enumerate() {
            let y = self.first + n * self.step;
            let tiles = &self.redraw[(y / TILE) * self.tiles_x..][..self.tiles_x];
            for (tx, _) in tiles.iter().enumerate().filter(|(_, d)| **d) {
                let x1 = ((tx + 1) * TILE).min(row.color.len());
                for x in tx * TILE..x1 {
                    let c = row.color[x];
                    let o = x * 4;
                    if c[3] > 0. {
                        for k in 0..3 {
                            row.rgba[o + k] = ((c[k] / c[3]).min(1.) * 255. + 0.5) as u8;
                        }
                        row.rgba[o + 3] = (c[3].min(1.) * 255. + 0.5) as u8;
                    } else {
                        row.rgba[o..o + 4].fill(0);
                    }
                }
            }
        }
    }

    /// Each pixel centre of this band's rows, in a redrawn tile, inside the
    /// triangle.
    fn raster(&mut self, tri: &Tri) {
        let (p, b, area) = (&tri.p, tri.b, tri.area);
        // The weights are the edge values over the area, so a pixel is
        // inside when all three edge values have the area's sign (or are
        // zero); only those pay for the divisions.
        let positive = area > 0.;
        // This band's first row at or after the triangle's top.
        let start = b[1].max(self.first);
        let start = start + (self.step - (start - self.first) % self.step) % self.step;
        for y in (start..b[3].min(self.end)).step_by(self.step) {
            let row = &mut self.rows[(y - self.first) / self.step];
            let tiles = &self.redraw[(y / TILE) * self.tiles_x..][..self.tiles_x];
            let first_tile = b[0] / TILE;
            let over = &tiles[first_tile..b[2].div_ceil(TILE)];
            for (n, _) in over.iter().enumerate().filter(|(_, r)| **r) {
                let tx = first_tile + n;
                for x in b[0].max(tx * TILE)..b[2].min((tx + 1) * TILE) {
                    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                    let edge = |a: [f32; 2], c: [f32; 2]| {
                        (c[0] - a[0]) * (py - a[1]) - (px - a[0]) * (c[1] - a[1])
                    };
                    let e = [edge(p[1], p[2]), edge(p[2], p[0]), edge(p[0], p[1])];
                    let inside = if positive {
                        e.iter().all(|&v| v >= 0.)
                    } else {
                        e.iter().all(|&v| v <= 0.)
                    };
                    if inside {
                        let w = e.map(|v| v / area);
                        pixel(row, &tri.fill, x, w);
                    }
                }
            }
        }
    }
}

fn pixel(row: &mut Row, fill: &Fill, i: usize, w: [f32; 3]) {
    let Fill::Sprite {
        quad,
        t,
        uv,
        z,
        texture,
    } = fill
    else {
        if let Fill::Mask(z) = fill {
            row.depth[i] = *z;
        }
        return;
    };
    if *z != 0 && *z <= row.depth[i] {
        return;
    }
    // Barycentric interpolation (same operation order as one sum).
    let mix = |v: [f32; 4]| v[t[0]] * w[0] + v[t[1]] * w[1] + v[t[2]] * w[2];
    let tex = texture.sample(mix(uv.map(|c| c[0])), mix(uv.map(|c| c[1])));
    let vc: [f32; 4] = std::array::from_fn(|ch| mix(quad.colors.map(|c| c[ch])));
    let a = (tex[3] * vc[3]).clamp(0., 1.);
    if a <= 0. {
        return;
    }
    let src: [f32; 3] = std::array::from_fn(|ch| (tex[ch] * vc[ch]).min(1.));
    let dst = &mut row.color[i];
    match quad.blend {
        1 => {
            for ch in 0..3 {
                dst[ch] = (dst[ch] + src[ch] * a).min(1.);
            }
            dst[3] = dst[3].max(a);
        }
        2 => {
            for ch in 0..3 {
                dst[ch] = (dst[ch] - src[ch] * a).max(0.);
            }
        }
        _ => {
            for ch in 0..3 {
                dst[ch] = src[ch] * a + dst[ch] * (1. - a);
            }
            dst[3] = a + dst[3] * (1. - a);
        }
    }
}

/// The canvas as it was before tiles, kept to prove the current one draws
/// the same bytes.
#[cfg(test)]
#[allow(dead_code)]
mod reference {
    use super::*;
    /// The rasterizer before tiles (the oracle for the current one): for the gauge: a straight-alpha RGBA canvas plus a
    /// depth buffer emulating the GS depth trick (masks write depth only;
    /// depth-tested sprites draw where their z is greater).
    pub struct RefCanvas {
        pub width: usize,
        pub height: usize,
        /// Canvas pixels per layout unit.
        pub scale: f32,
        pub rgba: Vec<u8>,
        color: Vec<[f32; 4]>,
        depth: Vec<i32>,
        dirty: Option<[usize; 4]>,
        /// Threads for a large frame.
        pub threads: usize,
    }

    impl RefCanvas {
        pub fn new(scale: f32) -> Self {
            let width = (CANVAS_WIDTH * scale).ceil() as usize;
            let height = (CANVAS_HEIGHT * scale).ceil() as usize;
            RefCanvas {
                width,
                height,
                scale,
                rgba: vec![0; width * height * 4],
                color: vec![[0.; 4]; width * height],
                depth: vec![0; width * height],
                dirty: None,
                threads: std::thread::available_parallelism().map_or(1, |n| n.get().min(8)),
            }
        }

        fn clear(&mut self) {
            if let Some([x0, y0, x1, y1]) = self.dirty.take() {
                for y in y0..y1 {
                    let row = y * self.width;
                    self.color[row + x0..row + x1].fill([0.; 4]);
                    self.depth[row + x0..row + x1].fill(0);
                    self.rgba[(row + x0) * 4..(row + x1) * 4].fill(0);
                }
            }
        }

        /// Clears last frame's area and draws this frame; `rgba` then holds
        /// straight-alpha pixels. Large frames (the full-screen iris and fade)
        /// draw in horizontal bands on several threads; every pixel still sees
        /// the draws in order, so the result does not depend on the split.
        pub fn render(&mut self, draws: &[Draw], textures: &[Texture]) {
            self.clear();
            let k = self.scale;
            let mut tris = Vec::new();
            for draw in draws {
                match draw {
                    Draw::Mask { polygons, z } => {
                        for q in polygons {
                            let q = q.map(|[x, y]| [(x + CANVAS_PAD) * k, y * k]);
                            for p in [[q[0], q[1], q[2]], [q[1], q[3], q[2]]] {
                                tris.extend(Tri::new(p, Fill::Mask(*z)));
                            }
                        }
                    }
                    Draw::Quads { quads, z, sheet } => {
                        let texture = &textures[*sheet];
                        for quad in quads {
                            let q = quad.corners.map(|[x, y]| [(x + CANVAS_PAD) * k, y * k]);
                            let [u0, v0, u1, v1] = quad.uv.map(|v| v as f32 * texture.texel_scale);
                            let uv = [[u0, v0], [u1, v0], [u0, v1], [u1, v1]];
                            for t in [[0usize, 1, 2], [1, 3, 2]] {
                                let fill = Fill::Sprite {
                                    quad,
                                    t,
                                    uv,
                                    z: *z,
                                    texture,
                                };
                                tris.extend(Tri::new(t.map(|i| q[i]), fill));
                            }
                        }
                    }
                }
            }
            // Pixel bounds clipped to the canvas; their union is this frame's
            // dirty area.
            let (width, height) = (self.width, self.height);
            tris.retain_mut(|tri| match bounds(&tri.p, width, height) {
                Some(b) => {
                    tri.b = b;
                    true
                }
                None => false,
            });
            for tri in &tris {
                let b = tri.b;
                self.dirty = Some(match self.dirty {
                    None => b,
                    Some(d) => [
                        d[0].min(b[0]),
                        d[1].min(b[1]),
                        d[2].max(b[2]),
                        d[3].max(b[3]),
                    ],
                });
            }
            let Some(dirty) = self.dirty else { return };
            let pixels = (dirty[2] - dirty[0]) * (dirty[3] - dirty[1]);
            let threads = if pixels >= 1 << 18 { self.threads } else { 1 };
            let rows = (dirty[3] - dirty[1]).div_ceil(threads);
            let first = dirty[1] * width;
            let bands = self.color[first..]
                .chunks_mut(rows * width)
                .zip(self.depth[first..].chunks_mut(rows * width))
                .zip(self.rgba[first * 4..].chunks_mut(rows * width * 4))
                .enumerate()
                .take(threads)
                .map(|(n, ((color, depth), rgba))| Band {
                    y0: dirty[1] + n * rows,
                    y1: (dirty[1] + (n + 1) * rows).min(dirty[3]),
                    width,
                    color,
                    depth,
                    rgba,
                });
            let tris = &tris;
            if threads == 1 {
                for mut band in bands {
                    band.draw(tris, dirty);
                }
            } else {
                std::thread::scope(|s| {
                    for mut band in bands {
                        s.spawn(move || band.draw(tris, dirty));
                    }
                });
            }
        }
    }

    /// Pixel bounds of a triangle clipped to the canvas.
    fn bounds(p: &[[f32; 2]; 3], width: usize, height: usize) -> Option<[usize; 4]> {
        let min = |k: usize| p.iter().map(|v| v[k]).fold(f32::MAX, f32::min);
        let max = |k: usize| p.iter().map(|v| v[k]).fold(f32::MIN, f32::max);
        let x0 = min(0).floor().max(0.) as usize;
        let y0 = min(1).floor().max(0.) as usize;
        let x1 = (max(0).ceil().max(0.) as usize).min(width);
        let y1 = (max(1).ceil().max(0.) as usize).min(height);
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        Some([x0, y0, x1, y1])
    }

    /// What a triangle writes: depth only (a mask) or a textured sprite.
    enum Fill<'a> {
        Mask(i32),
        Sprite {
            quad: &'a Quad,
            /// The quad corners of this triangle.
            t: [usize; 3],
            uv: [[f32; 2]; 4],
            z: i32,
            texture: &'a Texture<'a>,
        },
    }

    struct Tri<'a> {
        p: [[f32; 2]; 3],
        area: f32,
        /// Pixel bounds (set once clipped).
        b: [usize; 4],
        fill: Fill<'a>,
    }

    impl<'a> Tri<'a> {
        /// None for a degenerate triangle.
        fn new(p: [[f32; 2]; 3], fill: Fill<'a>) -> Option<Self> {
            let area = (p[1][0] - p[0][0]) * (p[2][1] - p[0][1])
                - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1]);
            (area.abs() >= 1e-6).then_some(Self {
                p,
                area,
                b: [0; 4],
                fill,
            })
        }
    }

    /// Canvas rows `y0..y1`; the slices start at row `y0`.
    struct Band<'a> {
        y0: usize,
        y1: usize,
        width: usize,
        color: &'a mut [[f32; 4]],
        depth: &'a mut [i32],
        rgba: &'a mut [u8],
    }

    impl Band<'_> {
        fn draw(&mut self, tris: &[Tri], dirty: [usize; 4]) {
            for tri in tris {
                self.raster(tri);
            }
            for y in self.y0..self.y1 {
                for x in dirty[0]..dirty[2] {
                    let i = (y - self.y0) * self.width + x;
                    let c = self.color[i];
                    let o = i * 4;
                    if c[3] > 0. {
                        for k in 0..3 {
                            self.rgba[o + k] = ((c[k] / c[3]).min(1.) * 255. + 0.5) as u8;
                        }
                        self.rgba[o + 3] = (c[3].min(1.) * 255. + 0.5) as u8;
                    } else {
                        self.rgba[o..o + 4].fill(0);
                    }
                }
            }
        }

        /// Each pixel centre of this band inside the triangle.
        fn raster(&mut self, tri: &Tri) {
            let (p, b, area) = (&tri.p, tri.b, tri.area);
            for y in b[1].max(self.y0)..b[3].min(self.y1) {
                for x in b[0]..b[2] {
                    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                    let edge = |a: [f32; 2], c: [f32; 2]| {
                        (c[0] - a[0]) * (py - a[1]) - (px - a[0]) * (c[1] - a[1])
                    };
                    let w = [
                        edge(p[1], p[2]) / area,
                        edge(p[2], p[0]) / area,
                        edge(p[0], p[1]) / area,
                    ];
                    if w.iter().all(|&v| v >= 0.) {
                        self.pixel(&tri.fill, (y - self.y0) * self.width + x, w);
                    }
                }
            }
        }

        fn pixel(&mut self, fill: &Fill, i: usize, w: [f32; 3]) {
            let Fill::Sprite {
                quad,
                t,
                uv,
                z,
                texture,
            } = fill
            else {
                if let Fill::Mask(z) = fill {
                    self.depth[i] = *z;
                }
                return;
            };
            if *z != 0 && *z <= self.depth[i] {
                return;
            }
            // Barycentric interpolation (same operation order as one sum).
            let mix = |v: [f32; 4]| v[t[0]] * w[0] + v[t[1]] * w[1] + v[t[2]] * w[2];
            let tex = sample(texture, mix(uv.map(|c| c[0])), mix(uv.map(|c| c[1])));
            let vc: [f32; 4] = std::array::from_fn(|ch| mix(quad.colors.map(|c| c[ch])));
            let a = (tex[3] * vc[3]).clamp(0., 1.);
            if a <= 0. {
                return;
            }
            let src: [f32; 3] = std::array::from_fn(|ch| (tex[ch] * vc[ch]).min(1.));
            let dst = &mut self.color[i];
            match quad.blend {
                1 => {
                    for ch in 0..3 {
                        dst[ch] = (dst[ch] + src[ch] * a).min(1.);
                    }
                    dst[3] = dst[3].max(a);
                }
                2 => {
                    for ch in 0..3 {
                        dst[ch] = (dst[ch] - src[ch] * a).max(0.);
                    }
                }
                _ => {
                    for ch in 0..3 {
                        dst[ch] = src[ch] * a + dst[ch] * (1. - a);
                    }
                    dst[3] = a + dst[3] * (1. - a);
                }
            }
        }
    }

    fn sample(t: &Texture, u: f32, v: f32) -> [f32; 4] {
        let [tw, th] = t.size;
        let (u, v) = (u - 0.5, v - 0.5);
        let (x0, y0) = (u.floor(), v.floor());
        let (fx, fy) = (u - x0, v - y0);
        let at = |x: f32, y: f32| {
            let x = (x.max(0.) as usize).min(tw - 1);
            let y = (y.max(0.) as usize).min(th - 1);
            let o = (y * tw + x) * 4;
            [0, 1, 2, 3].map(|c| t.rgba[o + c] as f32 / 255.)
        };
        let (a, b, c, d) = (
            at(x0, y0),
            at(x0 + 1., y0),
            at(x0, y0 + 1.),
            at(x0 + 1., y0 + 1.),
        );
        std::array::from_fn(|i| {
            let top = a[i] + (b[i] - a[i]) * fx;
            let bottom = c[i] + (d[i] - c[i]) * fx;
            top + (bottom - top) * fy
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_hide_depth_tested_pixels_only() {
        let mut canvas = Canvas::new(1.);
        let white = [255u8; 16];
        let texture = Texture {
            rgba: &white,
            size: [2, 2],
            texel_scale: 1.,
        };
        let square = |x: f32| Quad {
            corners: [[x, 0.], [x + 4., 0.], [x, 4.], [x + 4., 4.]],
            uv: [0, 0, 2, 2],
            colors: [[1.; 4]; 4],
            blend: 0,
        };
        let draws = [
            Draw::Mask {
                polygons: vec![[[0., 0.], [4., 0.], [0., 4.], [4., 4.]]],
                z: 200,
            },
            Draw::Quads {
                quads: vec![square(0.), square(8.)],
                z: 100,
                sheet: 0,
            },
            Draw::Quads {
                quads: vec![square(0.)],
                z: 300,
                sheet: 0,
            },
        ];
        canvas.render(&draws, std::slice::from_ref(&texture));
        let pad = CANVAS_PAD.ceil() as usize;
        let alpha = |c: &Canvas, x: usize, y: usize| c.rgba[(y * c.width + x + pad) * 4 + 3];
        // z 100 is hidden by the z 200 mask; z 300 shows; unmasked shows.
        assert_eq!(alpha(&canvas, 1, 1), 255);
        assert_eq!(alpha(&canvas, 9, 1), 255);
        canvas.render(&draws[..2], std::slice::from_ref(&texture));
        assert_eq!(alpha(&canvas, 1, 1), 0);
        assert_eq!(alpha(&canvas, 9, 1), 255);
    }

    #[test]
    fn banded_frames_match_one_thread() {
        let texture = Texture {
            rgba: &[
                255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 255, 255, 255, 255, 0,
            ],
            size: [2, 2],
            texel_scale: 1.,
        };
        let full = |blend, alpha| Quad {
            corners: [[-90., 0.], [600., 0.], [-90., 416.], [600., 416.]],
            uv: [0, 0, 2, 2],
            colors: [
                [1., 1., 1., alpha],
                [0.5, 1., 1., 1.],
                [1., 0.2, 1., 1.],
                [1.; 4],
            ],
            blend,
        };
        let draws = [
            Draw::Quads {
                quads: vec![full(0, 1.), full(1, 0.5)],
                z: 0,
                sheet: 0,
            },
            Draw::Mask {
                polygons: vec![[[10., 10.], [300., 40.], [20., 380.], [400., 300.]]],
                z: 200,
            },
            Draw::Quads {
                quads: vec![full(0, 0.7), full(2, 0.3)],
                z: 100,
                sheet: 0,
            },
        ];
        let mut one = Canvas::new(1.);
        one.threads = 1;
        let mut many = Canvas::new(1.);
        many.threads = 5;
        for _ in 0..2 {
            one.render(&draws, std::slice::from_ref(&texture));
            many.render(&draws, std::slice::from_ref(&texture));
            assert!(one.rgba == many.rgba);
        }
        assert!(one.rgba.iter().any(|&b| b != 0));
    }

    /// Random quads, masks and blends over several frames (so the clears
    /// are exercised), on one and on several threads.
    #[test]
    fn tiles_and_skips_draw_the_same_bytes_as_before() {
        let seed = std::cell::Cell::new(0x9e37_79b9_u32);
        let next = || {
            let mut s = seed.get();
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            seed.set(s);
            s
        };
        let texels: Vec<u8> = (0..32 * 32 * 4).map(|_| next() as u8).collect();
        let texture = Texture {
            rgba: &texels,
            size: [32, 32],
            texel_scale: 2.,
        };
        for scale in [1., 2.] {
            let mut tiled = Canvas::new(scale);
            let mut many = Canvas::new(scale);
            many.threads = 4;
            let mut before = reference::RefCanvas::new(scale);
            before.threads = 1;
            let mut mirror = tiled.rgba.clone();
            for frame in 0..12 {
                let f = || (next() % 10_000) as f32 / 10_000.;
                let mut draws = Vec::new();
                for _ in 0..(1 + frame % 4) {
                    let quads = (0..(2 + frame * 3))
                        .map(|_| {
                            let (x, y) = (f() * 700. - 100., f() * 460. - 20.);
                            let (w, h) = (f() * if frame % 5 == 0 { 600. } else { 60. }, f() * 50.);
                            Quad {
                                corners: [
                                    [x, y],
                                    [x + w, y + f() * 9.],
                                    [x - f() * 7., y + h],
                                    [x + w, y + h],
                                ],
                                uv: [0, 0, 16, 16].map(|v: i32| v + (next() % 8) as i32),
                                colors: [
                                    [f(), f(), f(), f() * 1.2],
                                    [1.; 4],
                                    [f(); 4],
                                    [0.5, 1., f(), 1.],
                                ],
                                blend: (next() % 3) as i32,
                            }
                        })
                        .collect();
                    draws.push(Draw::Quads {
                        quads,
                        z: [0, 100, 300][(next() % 3) as usize],
                        sheet: 0,
                    });
                    if next() % 2 == 0 {
                        let (x, y) = (f() * 500., f() * 400.);
                        draws.push(Draw::Mask {
                            polygons: vec![[
                                [x, y],
                                [x + 80., y + 5.],
                                [x, y + 70.],
                                [x + 90., y + 60.],
                            ]],
                            z: 200,
                        });
                    }
                }
                let slice = std::slice::from_ref(&texture);
                // Each frame, then the same frame with one quad moved (most
                // tiles keep their pixels).
                for nudge in [false, true] {
                    if nudge && let Some(Draw::Quads { quads, .. }) = draws.first_mut() {
                        quads[0].corners.iter_mut().for_each(|c| c[1] += 3.);
                    }
                    before.render(&draws, slice);
                    // (A nudge may move a quad that draws nothing.)
                    assert!(tiled.render(&draws, slice) || nudge);
                    tiled.copy_redrawn(&mut mirror);
                    // The redrawn rectangles carry the same bytes.
                    for ([x, y, w, h], rgba) in tiled.redrawn() {
                        let (x, y, w, h) = (x as usize, y as usize, w as usize, h as usize);
                        for r in 0..h {
                            let at = ((y + r) * tiled.width + x) * 4;
                            assert!(rgba[r * w * 4..(r + 1) * w * 4] == tiled.rgba[at..at + w * 4]);
                        }
                    }
                    many.render(&draws, slice);
                    assert!(tiled.rgba == before.rgba, "scale {scale} frame {frame}");
                    assert!(mirror == tiled.rgba, "copied tiles, frame {frame}");
                    assert!(
                        many.rgba == before.rgba,
                        "threads, scale {scale} frame {frame}"
                    );
                    // The same input again changes nothing.
                    assert!(!tiled.render(&draws, slice));
                    assert!(tiled.rgba == before.rgba);
                    assert!(before.rgba.iter().any(|&b| b != 0));
                }
            }
        }
    }

    #[test]
    fn low_hp_follows_integer_quarter() {
        let stats = |hp| GaugeStats {
            hp,
            max_hp: 20,
            mp: 100,
            max_mp: 100,
            drive_mode: 1,
            drive: 0,
            drive_level: 0,
            drive_max: 3,
            mp_charge: None,
            form_bars: 0,
            form_time: 0.,
            form_max_time: 0.,
        };
        assert_eq!(Gauge::state_of(&stats(5)), 1);
        assert_eq!(Gauge::state_of(&stats(6)), 0);
    }
}
