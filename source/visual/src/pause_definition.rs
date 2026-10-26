//! The pause-definition list (menu mode 8, `FUN_1402e4c20`): a backdrop
//! group, a vertical list of boxed items with the glove and the light, and
//! an optional 3D model drawn between the backdrop and the list. It is the
//! cutscene pause and the game-over list. All art is `pause.2ld` SEQD 0 on
//! image 0 in layout space. Construction `FUN_14031ef00` / `FUN_14031f1f0`,
//! backdrop `FUN_14031d3c0`, per-frame `FUN_14031e4b0`, result handler
//! `FUN_14031dd70`.
use crate::field_command_menu::Font;
use crate::sprite_player::{Id, LightCur, Sheet, Sprites, Text};
use data::pause_definition::Definition;
use data::sprite_sequence::{Quad, Sequence};

/// Sprite priorities: the backdrop layout 0x7531, the model's placeholder
/// (`DAT_140743710 - 1`), the list items (`DAT_140743710` = 0x753c), the
/// glove and the light 0x754a.
pub const PRIO_BACKDROP: u32 = 0x7531;
pub const PRIO_MODEL: u32 = 0x753b;
pub const PRIO_LIST: u32 = 0x753c;
pub const PRIO_POINTER: u32 = 0x754a;

/// SEQD 0 groups: the list items (`FUN_14031f1f0` creates them in group 0xc
/// and the selected one plays 0xb), the glove, the light's appear / loop,
/// and the placeholder the model attaches to (`FUN_14031ef00` group 0x10).
const ROW_IDLE: usize = 12;
const ROW_SELECTED: usize = 11;
const POINTER: usize = 6;
const LIGHT: [usize; 2] = [7, 8];
const MODEL_PLACEHOLDER: usize = 16;

/// The `pause.2ld` layout's centre anchor (the group 10 prop's translate)
/// the list and the model hang from.
pub const ANCHOR: [f32; 2] = [256., 216.];
/// The system font height of a list item (the group's TextScale is 0).
const TEXT_SIZE: f32 = 15.;

/// se000 sounds of the list (`FUN_1402eb740` ids through `FUN_1402f6270`):
/// the open sound `T[0xb]`, cursor and confirm.
pub const SOUND_OPEN: u32 = 3;
pub const SOUND_MOVE: u32 = 1;
pub const SOUND_DECIDE: u32 = 2;

/// Logical bits of pad mode 7.
const UP: u64 = 0x10;
const DOWN: u64 = 0x20;
const DECIDE: u64 = 0x4;

/// What a frame of the menu produced.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Output {
    /// se000 sounds to play this frame.
    pub sounds: Vec<u32>,
    /// The result id of the item chosen this frame.
    pub picked: Option<u16>,
}

/// The two draw bands around the 3D model's slot.
#[derive(Default)]
pub struct Draws {
    /// Backdrop and the model's placeholder.
    pub below: Vec<(Sheet, Vec<data::sprite_sequence::Quad>)>,
    /// The list, the glove and the light.
    pub above: Vec<(Sheet, Vec<data::sprite_sequence::Quad>)>,
}

pub struct Menu {
    sprites: Sprites,
    sheet: usize,
    results: Vec<u16>,
    texts: Vec<Vec<u8>>,
    backdrop: Option<usize>,
    with_model: bool,
    exclusive: bool,
    rows: Vec<Id>,
    cursor: usize,
    pointer: Option<Id>,
    light: LightCur,
    frames: u32,
}

impl Menu {
    /// `texts` holds each listed item's message; items with a zero result
    /// are skipped by the list builder (`FUN_14031f1f0`).
    pub fn new(
        definition: &Definition,
        texts: Vec<Vec<u8>>,
        sheet: usize,
        with_model: bool,
        last: Option<usize>,
    ) -> Self {
        let (results, texts): (Vec<u16>, Vec<Vec<u8>>) = definition
            .items
            .iter()
            .zip(texts)
            .filter(|(item, _)| item.result != 0)
            .map(|(item, text)| (item.result, text))
            .unzip();
        let cursor = last.filter(|&c| c < results.len()).unwrap_or(0);
        Self {
            sprites: Sprites::default(),
            sheet,
            results,
            texts,
            backdrop: usize::try_from(definition.backdrop).ok(),
            with_model,
            exclusive: definition.exclusive(),
            rows: Vec::new(),
            cursor,
            pointer: None,
            light: LightCur::default(),
            frames: 0,
        }
    }

    /// The cursor row (the exe keeps it for the next menu).
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// A row's text changes in place (another menu's value that Left and
    /// Right step; the game-over list never changes its own).
    pub fn set_text(&mut self, i: usize, text: Vec<u8>) {
        if self.texts.get(i) == Some(&text) {
            return;
        }
        if let Some(&row) = self.rows.get(i)
            && let Some(t) = self.sprites.get_mut(row).text.as_mut()
        {
            t.bytes = text.clone();
        }
        if let Some(t) = self.texts.get_mut(i) {
            *t = text;
        }
    }

    fn row_group(&self, i: usize) -> usize {
        if i == self.cursor {
            ROW_SELECTED
        } else {
            ROW_IDLE
        }
    }

    /// `FUN_14031d3c0` and `FUN_14031ef00`: the backdrop, the model's
    /// placeholder and the items stacked about the anchor (`FUN_14031db80`:
    /// each pitch is the group's UiPadding, the stack's top half its sum).
    fn build(&mut self, seq: &Sequence) {
        if let Some(group) = self.backdrop {
            self.sprites
                .create(seq, PRIO_BACKDROP, self.sheet, group, None, None);
        }
        if self.with_model {
            self.sprites
                .create(seq, PRIO_MODEL, self.sheet, MODEL_PLACEHOLDER, None, None);
        }
        let pitch = seq.groups[ROW_IDLE].ui_padding as f32;
        let n = self.results.len();
        for i in 0..n {
            let mut text = Text::new(self.texts[i].clone(), TEXT_SIZE);
            // Centred on the box: the group's light position (70) is the
            // light's, not the text's.
            text.align = 2;
            text.group_position = false;
            let id = self.sprites.create(
                seq,
                PRIO_LIST,
                self.sheet,
                self.row_group(i),
                None,
                Some(text),
            );
            let s = self.sprites.get_mut(id);
            s.base = ANCHOR;
            s.offset = [0., -(pitch * n as f32) / 2. + pitch * i as f32];
            self.rows.push(id);
        }
    }

    /// The glove and the light on the cursor row (`FUN_14031e4b0`).
    fn attach_pointer(&mut self, seq: &Sequence) {
        let Some(&row) = self.rows.get(self.cursor) else {
            return;
        };
        let p = self
            .sprites
            .create(seq, PRIO_POINTER, self.sheet, POINTER, None, None);
        self.sprites.get_mut(p).parent = Some(row);
        self.pointer = Some(p);
        let l = self.sprites.create(
            seq,
            PRIO_POINTER,
            self.sheet,
            LIGHT[0],
            Some(LIGHT[1]),
            None,
        );
        self.light.id = Some(l);
    }

    /// The sprites' own tasks, run before the menu task; the light follows
    /// the cursor row's context plus the group's light position.
    pub fn update_sprites(&mut self, seq: &Sequence) {
        self.sprites.update(seq);
        if self.light.id.is_some()
            && let Some(&row) = self.rows.get(self.cursor)
        {
            let c = self.sprites.context(row);
            let g = &seq.groups[self.sprites.get(row).player.group];
            self.light.place(
                &mut self.sprites,
                [
                    c[0] + g.light_position_x as f32,
                    c[1] + g.text_position_y as f32,
                ],
            );
        }
    }

    /// One frame of the menu task. `trig` holds this frame's new presses,
    /// `repeat` the pad's auto-repeat bits.
    pub fn frame(&mut self, seq: &Sequence, trig: u64, repeat: u64) -> Output {
        let mut out = Output::default();
        self.frames += 1;
        match self.frames {
            1 => {
                self.build(seq);
                out.sounds.push(SOUND_OPEN);
                return out;
            }
            2 => self.attach_pointer(seq),
            _ => {}
        }
        let n = self.rows.len();
        let before = self.cursor;
        if repeat & UP != 0 {
            self.cursor = self.cursor.saturating_sub(1);
        } else if repeat & DOWN != 0 && self.cursor + 1 < n {
            self.cursor += 1;
        } else if trig & DECIDE != 0 && n > 0 {
            out.sounds.push(SOUND_DECIDE);
            out.picked = Some(self.results[self.cursor]);
        }
        if self.cursor != before {
            out.sounds.push(SOUND_MOVE);
            for (i, &row) in self.rows.clone().iter().enumerate() {
                let group = self.row_group(i);
                self.sprites.set(seq, row, group, None);
            }
            if let (Some(p), Some(&row)) = (self.pointer, self.rows.get(self.cursor)) {
                self.sprites.get_mut(p).parent = Some(row);
            }
            self.light.retarget();
        }
        out
    }

    /// Whether Cancel and Start are ignored (they never close the
    /// game-over list: `FUN_1402f8d50` clears their enable bits).
    pub fn exclusive(&self) -> bool {
        self.exclusive
    }

    /// The draw lists around the model's slot.
    pub fn draw(&self, seq: &Sequence, font: &Font) -> Draws {
        Draws {
            below: self.sprites.draw_where(seq, font, |p| p <= PRIO_MODEL),
            above: self.sprites.draw_where(seq, font, |p| p > PRIO_MODEL),
        }
    }

    /// The model's placeholder context in layout space (the model is
    /// anchored where the group's prop sits).
    pub fn model_anchor(&self) -> [f32; 2] {
        ANCHOR
    }
}

/// `quads` of one full-layout 3D sheet shifted by `offset` from the layout
/// centre (the projection centre, layout (256, 208)).
pub fn model_quad(offset: [f32; 2], tint: f32, alpha: f32) -> Vec<Quad> {
    let (l, r) = (256. - 512. * 2. / 3., 256. + 512. * 2. / 3.);
    let [dx, dy] = offset;
    vec![Quad {
        corners: [
            [l + dx, dy],
            [r + dx, dy],
            [l + dx, 416. + dy],
            [r + dx, 416. + dy],
        ],
        uv: [0, 0, 1, 1],
        colors: [[tint, tint, tint, alpha]; 4],
        blend: 0,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::pause_definition::{Definition, Item};
    use data::sprite_sequence::AnimationGroup;

    /// A sequence with 19 empty groups, row groups 11 / 12 padded 32.
    fn sequence() -> Sequence {
        let group = |pad: i32| AnimationGroup {
            animations: 0..0,
            play_count: 0,
            loop_frames: [0, 0],
            light_position_x: 70,
            text_position_y: 0,
            text_scale: 0,
            ui_padding: pad,
            text_position_x: 0,
        };
        Sequence {
            sprites: Vec::new(),
            parts: Vec::new(),
            sprite_groups: Vec::new(),
            animations: Vec::new(),
            groups: (0..19)
                .map(|i| group(if i == 12 { 32 } else { 0 }))
                .collect(),
        }
    }

    fn definition(exclusive: bool) -> Definition {
        Definition {
            id: 1,
            items: vec![
                Item {
                    result: 2,
                    message: 0x8ab0,
                },
                Item {
                    result: 1,
                    message: 0x8aaf,
                },
            ],
            backdrop: 3,
            panel: -1,
            flags: [0, exclusive as u8, 1],
        }
    }

    fn menu(exclusive: bool) -> Menu {
        Menu::new(
            &definition(exclusive),
            vec![b"A".to_vec(), b"B".to_vec()],
            0,
            true,
            None,
        )
    }

    #[test]
    fn first_frame_builds_and_opens_with_the_open_sound() {
        let seq = sequence();
        let mut m = menu(true);
        let out = m.frame(&seq, 0, 0);
        assert_eq!(out.sounds, vec![SOUND_OPEN]);
        assert_eq!(m.rows.len(), 2);
        // Stacked about the anchor with the group's 32 pitch: -32 and 0.
        let y = |i: usize| m.sprites.get(m.rows[i]).offset[1];
        assert_eq!((y(0), y(1)), (-32., 0.));
        assert_eq!(m.sprites.get(m.rows[0]).base, ANCHOR);
        assert_eq!(m.sprites.get(m.rows[0]).player.group, ROW_SELECTED);
        assert_eq!(m.sprites.get(m.rows[1]).player.group, ROW_IDLE);
    }

    #[test]
    fn cursor_moves_without_wrapping_and_reports_the_result() {
        let seq = sequence();
        let mut m = menu(true);
        m.frame(&seq, 0, 0);
        m.frame(&seq, 0, 0);
        assert!(m.pointer.is_some());
        // Up at the top stays; down moves and plays the cursor sound.
        assert!(m.frame(&seq, UP, UP).sounds.is_empty());
        assert_eq!(m.frame(&seq, DOWN, DOWN).sounds, vec![SOUND_MOVE]);
        assert_eq!(m.cursor(), 1);
        assert!(m.frame(&seq, DOWN, DOWN).sounds.is_empty());
        assert_eq!(m.sprites.get(m.rows[1]).player.group, ROW_SELECTED);
        assert_eq!(m.sprites.get(m.rows[0]).player.group, ROW_IDLE);
        let out = m.frame(&seq, DECIDE, 0);
        assert_eq!(out.picked, Some(1));
        assert_eq!(out.sounds, vec![SOUND_DECIDE]);
    }

    #[test]
    fn cancel_and_start_do_nothing() {
        let seq = sequence();
        let mut m = menu(true);
        m.frame(&seq, 0, 0);
        m.frame(&seq, 0, 0);
        for bit in [0x8, 0x100] {
            assert_eq!(m.frame(&seq, bit, bit), Output::default());
        }
        assert!(m.exclusive());
    }

    #[test]
    fn the_last_chosen_row_starts_selected() {
        let seq = sequence();
        let mut m = Menu::new(
            &definition(true),
            vec![b"A".to_vec(), b"B".to_vec()],
            0,
            false,
            Some(1),
        );
        m.frame(&seq, 0, 0);
        assert_eq!(m.cursor(), 1);
        assert_eq!(m.sprites.get(m.rows[1]).player.group, ROW_SELECTED);
    }
}
