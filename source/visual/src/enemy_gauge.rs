//! The Scan target gauge, translated from the executable's GAUGE_ENEMY object
//! (vtable 0x1405b21e0, layer 6, 0x4b30 bytes): creation FUN_140182dd0, show
//! FUN_140183520, damage FUN_1401832a0, heal FUN_140183220, hide FUN_1401833e0,
//! per-frame FUN_1401835f0 / FUN_140183640, and the layout stacking offset
//! FUN_140182c60. It is shown by the lock manager (FUN_1403dd440) only while
//! the owner's ability word has bit 19 (Scan) and the target has an HP pool.
//!
//! The art is the `gaug` sequence's groups 38..=46 on the same sheet as the
//! player's gauge. HP is drawn in rows of 200: the top row as bars up to 100
//! art pixels scaled by the remainder, every completed row as a small tile
//! stepping left by the tile group's text X (11).
use crate::gauge::{Draw, X_ORIGIN};
use crate::hud_slots::HudSlots;
use data::sprite_sequence::{Modifiers, Player, Sequence};
use player::follow_camera::spring_with;

/// Root sprite (0x38, group 0x26): the label and the right edge every bar is
/// anchored to.
const ROOT: usize = 38;
/// Group 0x2a: a full 200-wide bar shown behind the top row when lower rows
/// are complete (0x230).
const FULL_ROW: usize = 42;
/// Group 0x27: the dark maximum bar (0x428).
const MAX_BAR: usize = 39;
/// Group 0x28: the cap at the maximum bar's left end (0x620).
const MAX_CAP: usize = 40;
/// Group 0x29: the damage trail, white flash then fade (0x818).
const SHADOW_BAR: usize = 41;
/// Group 0x2b: the current HP bar (0xa10).
const HP_BAR: usize = 43;
/// Segment tile pair per completed row: 0x2d dark, 0x2c lit, 0x2e fading.
const TILE_BACK: usize = 45;
const TILE_LIT: usize = 44;
const TILE_FADE: usize = 46;
/// Rows beyond this many tile pairs are not drawn.
const MAX_ROWS: usize = 16;
/// HP per row (the literal 200 in FUN_140183640).
const ROW_HP: i32 = 200;
/// The gauge's slot (0x12) and sub-slot (1) in the HUD stack (FUN_140182b10).
const SLOT: usize = 18;
const SUB: usize = 1;
/// Bar art width (100, 0x140623be4).
const BAR_ART: f32 = 100.;

struct Element {
    player: Player,
    parent: Option<usize>,
    offset: [f32; 2],
    hidden: bool,
    modifiers: Modifiers,
    /// This frame's child context and base, filled while drawing.
    context: [f32; 2],
    base: [f32; 2],
}

pub struct EnemyGauge {
    /// 0 root, 1 full row, 2 max bar, 3 max cap, 4 shadow, 5 hp, then pairs.
    elements: Vec<Element>,
    max: i32,
    hp: i32,
    shadow: i32,
    last: i32,
    rows: i32,
    max_rem: i32,
    shadow_rem: i32,
    hp_rem: i32,
    enabled: bool,
    frozen: bool,
    /// The stacking position (FUN_140182c60): the heights the lower HUD slots
    /// registered, followed by a spring. Float current (+0x30), previous
    /// (+0x34) and the integer base y (+0x2c).
    stack_current: f32,
    stack_previous: f32,
    stack_y: i32,
}

/// C integer division.
fn rows_of(hp: i32) -> i32 {
    (hp - 1) / ROW_HP
}

impl EnemyGauge {
    /// FUN_140182dd0: every sprite exists, hidden, and the layer is frozen.
    pub fn new(seq: &Sequence, slots: &mut HudSlots) -> Self {
        slots.register(SLOT, SUB, 0);
        let stack = slots.stack(SLOT, SUB);
        let mut elements = Vec::new();
        let mut push = |group: usize, parent: Option<usize>, offset: [f32; 2]| {
            elements.push(Element {
                player: Player::new(seq, group),
                parent,
                offset,
                hidden: true,
                modifiers: Modifiers::default(),
                context: [0.; 2],
                base: [0.; 2],
            });
        };
        push(ROOT, None, [0.; 2]);
        for group in [FULL_ROW, MAX_BAR, MAX_CAP, SHADOW_BAR, HP_BAR] {
            push(group, Some(0), [0.; 2]);
        }
        // Row tiles step left by the dark tile group's text X.
        let step = seq.groups[TILE_BACK].text_position_x as f32;
        for row in 0..MAX_ROWS {
            let offset = [-(step * row as f32), 0.];
            push(TILE_BACK, Some(0), offset);
            push(TILE_LIT, Some(0), offset);
        }
        Self {
            elements,
            max: 0,
            hp: 0,
            shadow: 0,
            last: 0,
            rows: 0,
            max_rem: 0,
            shadow_rem: 0,
            hp_rem: 0,
            enabled: false,
            frozen: true,
            stack_current: stack as f32,
            stack_previous: stack as f32,
            stack_y: stack,
        }
    }

    pub fn active(&self) -> bool {
        self.enabled
    }

    fn tile(&mut self, row: usize, lit: bool) -> &mut Element {
        &mut self.elements[6 + row * 2 + lit as usize]
    }

    /// FUN_140183520: show the gauge for a target's HP pool and run the first
    /// layout immediately.
    pub fn show(&mut self, seq: &Sequence, hp: i32, max: i32) {
        self.max = max;
        self.enabled = true;
        // FUN_140167070 / FUN_140167200: show and unfreeze the layer.
        self.frozen = false;
        for element in &mut self.elements {
            element.hidden = false;
        }
        self.set_hp(seq, hp);
        self.layout();
    }

    /// FUN_1401833e0.
    pub fn hide(&mut self) {
        self.enabled = false;
        for element in &mut self.elements {
            element.hidden = true;
        }
        self.frozen = true;
    }

    /// FUN_140183220: a heal sets the value (clamped to the maximum) without a
    /// damage trail.
    pub fn set_hp(&mut self, seq: &Sequence, hp: i32) {
        let hp = hp.min(self.max);
        self.hp = hp;
        self.last = hp;
        self.shadow = hp;
        for row in 0..rows_of(hp).max(0) as usize {
            if row < MAX_ROWS {
                self.tile(row, true).player = Player::new(seq, TILE_LIT);
            }
        }
    }

    /// FUN_1401832a0: damage starts the trail from the previous value, restarts
    /// the trail's flash and fades the lit tiles of the rows that emptied.
    pub fn damage(&mut self, seq: &Sequence, hp: i32) {
        self.shadow = self.last;
        let hp = hp.max(0);
        self.last = hp;
        self.hp = hp;
        self.elements[4].player = Player::new(seq, SHADOW_BAR);
        let lost = rows_of(self.shadow);
        for row in 0..lost.max(0) {
            let group = if rows_of(hp) <= row {
                TILE_FADE
            } else {
                TILE_LIT
            };
            if (row as usize) < MAX_ROWS {
                self.tile(row as usize, true).player = Player::new(seq, group);
            }
        }
    }

    /// FUN_140182c60: the base y follows the slot stack with the HUD spring
    /// (gain 0.4, spring 1.2, damping 0.5, rest 0.1).
    fn follow_stack(&mut self, slots: &HudSlots) {
        self.stack_previous = self.stack_current;
        let target = slots.stack(SLOT, SUB);
        if target != self.stack_y {
            let next = spring_with(
                [self.stack_current, 0., 0.],
                [target as f32, 0., 0.],
                [self.stack_previous, 0., 0.],
                0.4,
                [1.2, 0.5, 0.1],
            );
            self.stack_current = next[0];
            self.stack_y = next[0] as i32;
        }
    }

    /// FUN_140183640: sizes and shows the elements for the current values.
    fn layout(&mut self) {
        // FUN_140167070(6): every element of the layer is shown first.
        for element in &mut self.elements {
            element.hidden = false;
        }
        self.rows = rows_of(self.max);
        let mut max_rem = self.max - self.rows * ROW_HP;
        let hp_row = rows_of(self.hp);
        let hp_rem = self.hp - hp_row * ROW_HP;
        let shadow_row = rows_of(self.shadow);
        let mut shadow_rem = self.shadow - shadow_row * ROW_HP;
        if max_rem < hp_rem {
            max_rem = ROW_HP;
        }
        if self.rows != hp_row {
            max_rem = ROW_HP;
        }
        if shadow_row != hp_row {
            shadow_rem = ROW_HP;
        }
        self.max_rem = max_rem;
        self.shadow_rem = shadow_rem;
        self.hp_rem = hp_rem;
        if !(hp_row == self.rows && self.rows != 0) {
            self.elements[1].hidden = true;
        }
        for row in 0..MAX_ROWS {
            self.tile(row, false).hidden = true;
            self.tile(row, true).hidden = true;
        }
        for row in 0..self.rows.max(0) as usize {
            if row < MAX_ROWS {
                self.tile(row, false).hidden = false;
            }
        }
        let bar = |element: &mut Element, width: i32| {
            element.modifiers.scale_x = width as f32 / BAR_ART;
            element.offset = [-(width as f32), 0.];
        };
        bar(&mut self.elements[2], max_rem);
        self.elements[3].offset = [-(max_rem as f32), 0.];
        bar(&mut self.elements[4], shadow_rem);
        bar(&mut self.elements[5], hp_rem);
        if self.rows != 0 {
            for row in 0..shadow_row.max(0) as usize {
                if row < MAX_ROWS {
                    self.tile(row, true).hidden = false;
                }
            }
        }
    }

    /// One 60 Hz frame: the gauge's own logic (FUN_1401835f0), then each
    /// sprite's player in creation order.
    pub fn update(&mut self, seq: &Sequence, slots: &HudSlots) {
        if !self.enabled || self.frozen {
            return;
        }
        self.follow_stack(slots);
        self.layout();
        for element in &mut self.elements {
            element.player.update(seq, 1.);
        }
    }

    /// This frame's drawables in canvas layout units, with the HUD sheet index.
    pub fn draw(&mut self, seq: &Sequence, sheet: usize) -> Vec<Draw> {
        let mut out = Vec::new();
        if !self.enabled {
            return out;
        }
        for i in 0..self.elements.len() {
            let base = match self.elements[i].parent {
                Some(p) => {
                    let p = &self.elements[p];
                    [
                        p.context[0] + p.offset[0] + p.base[0],
                        p.context[1] + p.offset[1] + p.base[1],
                    ]
                }
                // FUN_14016b330: the root's base is the stacking position.
                None => [0., self.stack_y as f32],
            };
            let e = &mut self.elements[i];
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
                z: 0,
                sheet,
            });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::sprite_sequence::{Animation, AnimationGroup, Sprite, SpritePart};

    /// Every group plays one always-active animation drawing one part.
    fn synthetic() -> Sequence {
        let animation = Animation {
            flags: data::sprite_sequence::IS_ACTIVE | data::sprite_sequence::LINEAR,
            sprite_group: 0,
            frames: [0, 100],
            translate: [0; 4],
            pivot: [0; 4],
            rotation: [0.; 6],
            scale: [1.; 2],
            scale_x: [1.; 2],
            scale_y: [1.; 2],
            curve: [0.; 4],
            bounce: [0.; 4],
            bounce_count: [0; 2],
            blend: 0,
            color: [0x8080_8080; 2],
        };
        let group = AnimationGroup {
            animations: 0..1,
            play_count: 0,
            loop_frames: [0, 0],
            light_position_x: 0,
            text_position_y: 0,
            text_scale: 0,
            ui_padding: 0,
            text_position_x: 11,
        };
        Sequence {
            sprites: vec![Sprite {
                uv: [0, 0, 100, 10],
                scroll: [0.; 2],
                colors: [0x8080_8080; 4],
            }],
            parts: vec![SpritePart {
                rect: [0, -5, 100, 5],
                sprite: 0,
            }],
            sprite_groups: std::iter::once(0..1).collect(),
            animations: vec![animation],
            groups: vec![group; 47],
        }
    }

    fn visible(gauge: &EnemyGauge) -> Vec<usize> {
        (0..gauge.elements.len())
            .filter(|&i| !gauge.elements[i].hidden)
            .collect()
    }

    #[test]
    fn a_hidden_gauge_neither_updates_nor_draws() {
        let seq = synthetic();
        let mut gauge = EnemyGauge::new(&seq, &mut HudSlots::default());
        gauge.update(&seq, &HudSlots::default());
        assert!(gauge.draw(&seq, 0).is_empty());
        assert!(!gauge.active());
        assert_eq!(gauge.elements[0].player.time, 0.);
    }

    #[test]
    fn rows_of_two_hundred_size_the_top_row_and_the_tiles() {
        let seq = synthetic();
        let mut gauge = EnemyGauge::new(&seq, &mut HudSlots::default());
        gauge.show(&seq, 450, 1000);
        // 1000 HP: four completed rows plus a full top row; 450 HP sits in
        // the third row with 50 left in it.
        assert_eq!((gauge.rows, gauge.max_rem, gauge.hp_rem), (4, 200, 50));
        assert_eq!(gauge.elements[5].offset, [-50., 0.]);
        assert_eq!(gauge.elements[5].modifiers.scale_x, 0.5);
        assert_eq!(gauge.elements[2].offset, [-200., 0.]);
        // The full-row bar hides until the top row is the current one.
        assert!(gauge.elements[1].hidden);
        // Dark tiles for the four completed rows, lit tiles for the two
        // below the current row.
        let shown = visible(&gauge);
        for row in 0..4 {
            assert!(shown.contains(&(6 + row * 2)));
        }
        assert!(!shown.contains(&(6 + 4 * 2)));
        assert!(shown.contains(&(6 + 1)) && shown.contains(&(6 + 3)));
        assert!(!shown.contains(&(6 + 5)));
        // Tiles step left by the tile group's text X.
        assert_eq!(gauge.elements[6 + 2 * 3].offset, [-33., 0.]);
        let top = EnemyGauge::new(&seq, &mut HudSlots::default());
        assert!(top.elements.iter().all(|e| e.hidden));
        gauge.show(&seq, 1000, 1000);
        assert!(!gauge.elements[1].hidden);
    }

    #[test]
    fn damage_starts_a_trail_from_the_previous_value_and_fades_emptied_rows() {
        let seq = synthetic();
        let mut gauge = EnemyGauge::new(&seq, &mut HudSlots::default());
        gauge.show(&seq, 450, 1000);
        gauge.update(&seq, &HudSlots::default());
        gauge.damage(&seq, 250);
        gauge.update(&seq, &HudSlots::default());
        assert_eq!((gauge.hp, gauge.shadow), (250, 450));
        // The trail spans a full row because it is a row above the current.
        assert_eq!(gauge.shadow_rem, 200);
        assert_eq!(gauge.hp_rem, 50);
        assert_eq!(gauge.elements[4].player.group, SHADOW_BAR);
        // Rows 0 and 1 were lit; row 1 is above the current row now and fades.
        assert_eq!(gauge.elements[6 + 1].player.group, TILE_LIT);
        assert_eq!(gauge.elements[6 + 3].player.group, TILE_FADE);
        // Healing sets the value with no trail.
        gauge.set_hp(&seq, 700);
        assert_eq!((gauge.hp, gauge.shadow), (700, 700));
        gauge.set_hp(&seq, 5000);
        assert_eq!(gauge.hp, 1000);
    }

    #[test]
    fn zero_hp_leaves_an_empty_current_bar() {
        let seq = synthetic();
        let mut gauge = EnemyGauge::new(&seq, &mut HudSlots::default());
        gauge.show(&seq, 120, 120);
        gauge.damage(&seq, 0);
        gauge.update(&seq, &HudSlots::default());
        assert_eq!((gauge.hp, gauge.hp_rem, gauge.rows), (0, 0, 0));
        assert_eq!(gauge.shadow_rem, 120);
    }

    #[test]
    fn the_root_anchors_children_and_bars_grow_left() {
        let seq = synthetic();
        let mut gauge = EnemyGauge::new(&seq, &mut HudSlots::default());
        gauge.show(&seq, 100, 100);
        gauge.update(&seq, &HudSlots::default());
        let draws = gauge.draw(&seq, 3);
        assert!(!draws.is_empty());
        let Draw::Quads { sheet, .. } = &draws[0] else {
            panic!("root quads")
        };
        assert_eq!(*sheet, 3);
        gauge.hide();
        assert!(gauge.draw(&seq, 3).is_empty());
    }

    #[test]
    fn the_base_follows_the_registered_slot_heights_with_a_spring() {
        let seq = synthetic();
        let mut slots = HudSlots::default();
        let mut gauge = EnemyGauge::new(&seq, &mut slots);
        gauge.show(&seq, 100, 100);
        gauge.update(&seq, &slots);
        assert_eq!(gauge.stack_y, 0);
        // A popup in a lower slot of the same sub-slot pushes the gauge down.
        slots.register(3, SUB, 40);
        let mut last = 0;
        for _ in 0..80 {
            gauge.update(&seq, &slots);
            assert!(gauge.stack_y >= last && gauge.stack_y <= 40);
            last = gauge.stack_y;
        }
        assert!(last >= 38, "{last}");
        let Draw::Quads { quads, .. } = &gauge.draw(&seq, 0)[0] else {
            panic!("root quads")
        };
        // The root's rectangle starts 85 (origin) + 0 and sits `last` lower.
        assert!((quads[0].corners[0][1] - (-5. + last as f32)).abs() < 1.5);
        // Other sub-slots do not move it.
        slots.release(3, SUB);
        slots.register(3, 0, 90);
        for _ in 0..200 {
            gauge.update(&seq, &slots);
        }
        assert!(gauge.stack_y <= 1);
    }
}
