//! The world map's 2D screens on `file/<lang>/wm_ws.2ld`: the landing info
//! panel dk::WM_LANDING_INFO (FUN_140395970, closed by FUN_1403966c0) and
//! the landing menu dk::WM_LANDING's presentation (FUN_140390d00,
//! FUN_1403919e0, FUN_140390ff0, FUN_140392010, FUN_1403928a0,
//! FUN_1403931f0, FUN_140392430). The rules are `world_map::landing`.
//!
//! Every sprite uses the layout's SEQD 1 on image 1 except the world-point
//! pictures (70landing.bar's 'ws_i' SEQD on each picture). Tz::LaySeq and
//! Tz::LayMsg follow a prop of the layout's current scene (FUN_140194f20:
//! a group index across the layout's sequences), as FUN_14034b710 and
//! FUN_1402f9d30 do.
use crate::field_command_menu::Font;
use crate::sprite_player::{self, Id, LightCur, Sprites, Text, text_quads};
use data::menu_layout::{Layout, LayoutTask};
use data::sprite_sequence::{Modifiers, Player, Quad, Sequence};
use world_map::landing::{self, Change, Landing};

/// The SEQD and image every sprite draws from.
pub const SEQUENCE: usize = 1;
pub const IMAGE: usize = 1;

/// Draw priorities (FUN_14016a900's first argument).
const PRIO_LAYOUT: u32 = 0x7531;
const PRIO_TEXT: u32 = 0x7535;
const PRIO_CELL: u32 = 0x753a;
const PRIO_ROW: u32 = 0x7544;
const PRIO_BLOCKED: u32 = 0x754e;
const PRIO_PANEL: u32 = 0x7550;
const PRIO_EPISODE: u32 = 0x7554;
const PRIO_BADGE: u32 = 0x7556;
const PRIO_MARK: u32 = 0x7558;
const PRIO_CURSOR: u32 = 0x7562;
const PRIO_NOTICE: u32 = 0x756c;
const PRIO_INFO: u32 = 0x7585;

/// SEQD 1 groups.
const ROW: [usize; 3] = [0x3a, 0x3c, 0x3b];
const ROW_SELECTED: usize = 0x39;
const CELL: usize = 0x37;
const CELL_BLOCKED: usize = 0x38;
const CURSOR: [usize; 3] = [0xe, 0x10, 0xf];
/// FUN_1402f6610's two marks on the selected cell (slots 0 and 1).
const MARKS: [usize; 2] = [0x36, 0xd];
const NOTICE: [usize; 3] = [0x5b, 0x5c, 0x5d];
const PANEL: [usize; 4] = [0x5e, 0x5f, 0x60, 1];
const EPISODE_ROW: usize = 0x34;
const BADGE: [usize; 2] = [1, 0x11];
/// The picture frame of 70landing.bar's 'ws_i' SEQD.
const PICTURE: usize = 0xb;
/// Info panel groups (FUN_140395970) and their close groups (FUN_1403966c0).
const INFO_NAME: [usize; 3] = [0x98, 0x99, 0x9a];
const INFO_STAR: [usize; 2] = [0x95, 0x96];
const INFO_TIME: [usize; 3] = [0x48, 0x49, 0x4a];
const INFO_VALUE: [usize; 3] = [0x41, 0x42, 0x43];
const DIGIT_VALUE: usize = 0x12;
const DIGIT_TIME: usize = 0x1c;
const COLON: usize = 0x26;

/// Props the grid cells follow per mode ({open, close, idle} scene of the
/// mode: 0x1405c0f28 / f30 / f38 / f40, as flat group indices).
const CELL_PROPS: [[usize; 3]; 4] = [
    [0x7b, 0x7d, 0x7c],
    [0x7b, 0x7d, 0x7c],
    [0x7d, 0x16, 0x15],
    [0x7b, 0x7d, 0x7c],
];

/// The nine Tz::LayMsg texts (0x1405c0f90, 0x20 bytes each): their props in
/// modes 1, 2 and 3 (+0 / +8 / +0x10), the message (+0x18; None = set later)
/// and alignment (+0x1c).
struct LabelSpec {
    props: [[usize; 3]; 3],
    message: Option<u32>,
    align: u8,
}

const LABELS: [LabelSpec; 9] = [
    LabelSpec {
        props: [[0x58, 0x5a, 0x59], [0x5a, 0x16, 0x15], [0x58, 0x5a, 0x59]],
        message: None,
        align: 0,
    },
    LabelSpec {
        props: [[0x62, 0x64, 0x63], [0x64, 0x16, 0x15], [0x62, 0x64, 0x63]],
        message: Some(0x4c11),
        align: 2,
    },
    LabelSpec {
        props: [[0x66, 0x68, 0x67], [0x16, 0x16, 0x15], [0x66, 0x68, 0x67]],
        message: Some(0x4c14),
        align: 2,
    },
    LabelSpec {
        props: [[0x75, 0x77, 0x76], [0x77, 0x16, 0x15], [0x75, 0x77, 0x76]],
        message: Some(0x4c12),
        align: 2,
    },
    LabelSpec {
        props: [[0x78, 0x7a, 0x79], [0x7a, 0x16, 0x15], [0x78, 0x7a, 0x79]],
        message: Some(0x4c13),
        align: 2,
    },
    LabelSpec {
        props: [[0x16, 0x16, 0x15], [0x69, 0x6b, 0x6a], [0x6b, 0x16, 0x15]],
        message: Some(0x4c10),
        align: 2,
    },
    LabelSpec {
        props: [[0x7b, 0x7d, 0x7c], [0x7d, 0x16, 0x15], [0x7b, 0x7d, 0x7c]],
        message: Some(0x4c15),
        align: 2,
    },
    LabelSpec {
        props: [[0x75, 0x77, 0x76], [0x77, 0x16, 0x15], [0x75, 0x77, 0x76]],
        message: None,
        align: 0,
    },
    LabelSpec {
        props: [[0x78, 0x7a, 0x79], [0x7a, 0x16, 0x15], [0x78, 0x7a, 0x79]],
        message: None,
        align: 0,
    },
];
/// The message ids the menu's fixed labels use, for the caller to load.
pub fn label_messages() -> Vec<u32> {
    LABELS.iter().filter_map(|l| l.message).collect()
}

/// What a draw list item samples.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sheet {
    /// A 2LD image by index (the layout's props, and SEQD 1 sprites on
    /// `IMAGE`).
    Image(usize),
    /// World-point picture `n` of 70landing.bar.
    Picture(usize),
    Font,
    Icon,
}

/// A shown prop of the current scene: where its children sit, its colour
/// and group.
#[derive(Clone, Copy, Debug)]
struct Anchor {
    at: [f32; 2],
    color: [f32; 4],
    sequence: usize,
    group: usize,
}

/// FUN_140194f20(layout, flat, 0): the first shown prop of the task's scene
/// playing flat group `flat` (groups counted across the layout's SEQDs).
fn anchor(layout: &Layout, task: &LayoutTask, flat: usize) -> Option<Anchor> {
    if task.dead {
        return None;
    }
    let mut base = 0;
    let (sequence, group) = layout.sequences.iter().enumerate().find_map(|(k, s)| {
        let n = s.groups.len();
        let hit = (base..base + n).contains(&flat).then_some((k, flat - base));
        base += n;
        hit
    })?;
    let m = Modifiers::default();
    task.player
        .visible(layout, [0, 0])
        .find(|(p, _, _)| p.sequence == sequence && p.group == group)
        .map(|(_, player, origin)| {
            let seq = &layout.sequences[sequence];
            let c = seq.child_context(player, &m).unwrap_or([0.; 2]);
            Anchor {
                at: [origin[0] + c[0], origin[1] + c[1]],
                color: seq
                    .context_color(player, &m)
                    .or_else(|| seq.current_color(player))
                    .unwrap_or([1.; 4]),
                sequence,
                group,
            }
        })
}

/// One draw item with its priority.
type Item = (u32, Sheet, Vec<Quad>);

fn sprite_items(sprites: &Sprites, seq: &Sequence, font: &Font, out: &mut Vec<Item>) {
    let mut prios: Vec<u32> = sprites.list.iter().map(|s| s.prio).collect();
    prios.sort_unstable();
    prios.dedup();
    for prio in prios {
        for (sheet, quads) in sprites.draw_where(seq, font, |p| p == prio) {
            let sheet = match sheet {
                sprite_player::Sheet::Art(k) => Sheet::Image(k),
                sprite_player::Sheet::Font => Sheet::Font,
                sprite_player::Sheet::Icon => Sheet::Icon,
            };
            out.push((prio, sheet, quads));
        }
    }
}

/// Sorts the items by priority (stable) into a draw list.
fn ordered(mut items: Vec<Item>) -> Vec<(Sheet, Vec<Quad>)> {
    items.sort_by_key(|(p, _, _)| *p);
    items
        .into_iter()
        .filter(|(_, _, q)| !q.is_empty())
        .map(|(_, s, q)| (s, q))
        .collect()
}

fn text_size(seq: &Sequence, group: usize) -> f32 {
    match seq.groups[group].text_scale {
        s if s > 0 => s as f32,
        _ => 15.,
    }
}

/// dk::WM_LANDING_INFO: the world's name with its visit stars, the play
/// time and a two-digit value.
pub struct Info {
    pub world: u8,
    sprites: Sprites,
    /// The name, time and value frames (the ones FUN_1403966c0 closes).
    frames: [Id; 3],
    closing: bool,
}

impl Info {
    /// FUN_140395970. `visits` = (stars, stars lit) from 'wmky'
    /// (FUN_14038d330 / FUN_14038d2c0); `value` = the world's battle level
    /// (FUN_1403ee470; None leaves the digits out);
    /// `time` = the world's play time (hours, minutes, seconds).
    pub fn new(
        seq: &Sequence,
        world: u8,
        name: Vec<u8>,
        visits: Option<(usize, usize)>,
        value: Option<u8>,
        time: [u32; 3],
    ) -> Self {
        let mut sprites = Sprites::default();
        let mut t = Text::new(name, text_size(seq, INFO_NAME[0]));
        t.align = 2;
        // FUN_140166ba0: no group offset.
        t.group_position = false;
        let name = sprites.create(
            seq,
            PRIO_INFO,
            IMAGE,
            INFO_NAME[0],
            Some(INFO_NAME[1]),
            Some(t),
        );
        if let Some((count, lit)) = visits.filter(|v| v.0 > 0) {
            let first = INFO_STAR[usize::from(lit > 0)];
            let g = &seq.groups[first];
            let (x0, pitch) = (g.light_position_x as f32, g.text_scale as f32);
            for i in 0..count {
                let id = sprites.create(
                    seq,
                    PRIO_INFO,
                    IMAGE,
                    INFO_STAR[usize::from(i < lit)],
                    None,
                    None,
                );
                sprites.attach(id, name);
                sprites.get_mut(id).offset = [x0 - pitch * i as f32, 0.];
            }
            if let Some(t) = sprites.get_mut(name).text.as_mut() {
                t.offset[0] = -((pitch as i32 * count as i32) / 2) as f32;
            }
        }
        let time_frame = sprites.create(
            seq,
            PRIO_INFO,
            IMAGE,
            INFO_TIME[0],
            Some(INFO_TIME[1]),
            None,
        );
        let value_frame = sprites.create(
            seq,
            PRIO_INFO,
            IMAGE,
            INFO_VALUE[0],
            Some(INFO_VALUE[1]),
            None,
        );
        let row = |sprites: &mut Sprites, parent: Id, groups: &[usize], mut x: f32| {
            for &g in groups {
                let id = sprites.create(seq, PRIO_INFO, IMAGE, g, None, None);
                sprites.attach(id, parent);
                sprites.get_mut(id).offset = [x, 0.];
                x += seq.groups[g].text_position_x as f32;
            }
            x
        };
        if let Some(v) = value {
            let (d, _) = landing::digits(v as u32, 2);
            let groups: Vec<usize> = d.iter().map(|&d| DIGIT_VALUE + d as usize).collect();
            row(&mut sprites, value_frame, &groups, 0.);
        }
        let (h, used) = landing::digits(time[0], 3);
        let mut groups: Vec<usize> = h[3 - used..]
            .iter()
            .map(|&d| DIGIT_TIME + d as usize)
            .collect();
        groups.push(COLON);
        let (m, _) = landing::digits(time[1], 2);
        groups.extend(m.iter().map(|&d| DIGIT_TIME + d as usize));
        row(&mut sprites, time_frame, &groups, 0.);
        Self {
            world,
            sprites,
            frames: [name, time_frame, value_frame],
            closing: false,
        }
    }

    /// FUN_1403966c0: the frames play their close groups; the panel goes
    /// when they end.
    pub fn close(&mut self, seq: &Sequence) {
        if self.closing {
            return;
        }
        self.closing = true;
        for (id, g) in self
            .frames
            .into_iter()
            .zip([INFO_NAME[2], INFO_TIME[2], INFO_VALUE[2]])
        {
            self.sprites.close(seq, id, g);
        }
    }

    pub fn closing(&self) -> bool {
        self.closing
    }

    pub fn update(&mut self, seq: &Sequence) {
        self.sprites.update(seq);
    }

    /// Gone once every frame finished closing.
    pub fn done(&self) -> bool {
        self.closing && self.frames.iter().all(|&id| !self.sprites.alive(id))
    }

    pub fn draw(&self, seq: &Sequence, font: &Font) -> Vec<(Sheet, Vec<Quad>)> {
        let mut items = Vec::new();
        sprite_items(&self.sprites, seq, font, &mut items);
        ordered(items)
    }
}

/// Texts the landing menu shows, already resolved by the caller.
#[derive(Clone, Debug, Default)]
pub struct MenuTexts {
    /// The two rows of the two-row menu.
    pub rows: [Vec<u8>; 2],
    /// Per grid cell: its place name (FUN_1403bac20).
    pub cell_names: Vec<Vec<u8>>,
    /// The fixed labels in `label_messages()` order.
    pub labels: Vec<Vec<u8>>,
    /// The world's story summary and objective (FUN_140344660 through
    /// FUN_1402f23d0's journal index).
    pub story: [Vec<u8>; 2],
    pub notice: Vec<u8>,
    pub heading: Vec<u8>,
    /// Per episode: title and help.
    pub episodes: Vec<(Vec<u8>, Vec<u8>)>,
}

/// A Tz::LayMsg: a text on a prop of the current scene.
struct Label {
    props: [[usize; 3]; 3],
    text: Vec<u8>,
    align: u8,
}

/// A world-point picture on its cell (FUN_1402f75d0 on 'ws_i').
struct Picture {
    cell: Id,
    picture: usize,
    player: Player,
    hidden: bool,
}

/// dk::WM_LANDING's presentation.
pub struct Menu {
    pub task: LayoutTask,
    sprites: Sprites,
    /// Two-row menu rows.
    rows: Vec<Id>,
    /// Grid: per cell its sprite, blocked overlay and picture.
    cells: Vec<(Id, Option<Id>)>,
    pictures: Vec<Picture>,
    picture_seq: Option<Sequence>,
    marks: Vec<Id>,
    cursor: LightCur,
    labels: Vec<Label>,
    panel: Option<Id>,
    episode_rows: Vec<(Id, Id)>,
    notice: Option<Id>,
    texts: MenuTexts,
    /// The mode the props and scenes follow (0x2838 as last applied).
    mode: u8,
    last_cursor: Option<usize>,
    idle_reached: bool,
}

impl Menu {
    /// FUN_140390d00 with FUN_1403919e0 / FUN_140390ff0. `pictures` holds
    /// each cell's picture index (None for an empty cell) and `ws_i` the
    /// pictures' SEQD.
    pub fn new(
        layout: &Layout,
        state: &Landing,
        texts: MenuTexts,
        pictures: &[Option<usize>],
        ws_i: Option<Sequence>,
    ) -> Self {
        let seq = &layout.sequences[SEQUENCE];
        let Change::Scene { scene, next } = state.opening() else {
            unreachable!("a new menu opens with a scene")
        };
        let mut task = LayoutTask::new(layout, scene);
        task.next = Some(next);
        let mut menu = Self {
            task,
            sprites: Sprites::default(),
            rows: Vec::new(),
            cells: Vec::new(),
            pictures: Vec::new(),
            picture_seq: ws_i,
            marks: Vec::new(),
            cursor: LightCur::default(),
            labels: Vec::new(),
            panel: None,
            episode_rows: Vec::new(),
            notice: None,
            texts,
            mode: state.mode,
            last_cursor: None,
            idle_reached: false,
        };
        let cursor = menu
            .sprites
            .create(seq, PRIO_CURSOR, IMAGE, CURSOR[0], Some(CURSOR[2]), None);
        menu.cursor = LightCur::new(cursor);
        if state.mode == landing::ROWS {
            menu.build_rows(seq);
        } else {
            menu.build_grid(layout, state, pictures);
        }
        menu
    }

    /// FUN_1403919e0: the two rows, one row pitch apart, centred text.
    fn build_rows(&mut self, seq: &Sequence) {
        for (k, bytes) in self.texts.rows.clone().into_iter().enumerate() {
            let g = &seq.groups[ROW[0]];
            let mut t = Text::new(bytes, text_size(seq, ROW[0]));
            t.align = 2;
            t.offset = [-g.light_position_x as f32, -g.text_position_y as f32];
            let id = self
                .sprites
                .create(seq, PRIO_ROW, IMAGE, ROW[0], Some(ROW[2]), Some(t));
            self.sprites.get_mut(id).offset = [0., (g.text_position_x * k as i32) as f32];
            self.rows.push(id);
        }
    }

    /// FUN_140390ff0: the cells (2 columns from the cell prop's group origin,
    /// the cell group's pitches), their pictures, the labels and the
    /// episode panel.
    fn build_grid(&mut self, layout: &Layout, state: &Landing, pictures: &[Option<usize>]) {
        let seq = &layout.sequences[SEQUENCE];
        let g = &seq.groups[CELL];
        let (pitch_x, pitch_y) = (g.text_position_x as f32, g.ui_padding as f32);
        // The origin is the opening scene's cell prop's group position.
        let origin = {
            let mut base = 0;
            let flat = CELL_PROPS[1][0];
            let group = layout.sequences.iter().find_map(|s| {
                let n = s.groups.len();
                let hit = (base..base + n)
                    .contains(&flat)
                    .then(|| &s.groups[flat - base]);
                base += n;
                hit
            });
            group.map_or([0.; 2], |g| {
                [g.light_position_x as f32, g.text_position_y as f32]
            })
        };
        let fallback = pictures.iter().rev().flatten().next().copied();
        for (k, cell) in state.cells.iter().enumerate() {
            let (col, row) = ((k % 2) as f32, (k / 2) as f32);
            let at = [origin[0] + col * pitch_x, origin[1] + row * pitch_y];
            let id = self.sprites.create(seq, PRIO_CELL, IMAGE, CELL, None, None);
            self.sprites.get_mut(id).offset = at;
            let blocked = cell.is_some_and(|c| c.blocked.is_some()).then(|| {
                let b = self
                    .sprites
                    .create(seq, PRIO_BLOCKED, IMAGE, CELL_BLOCKED, None, None);
                self.sprites.get_mut(b).offset = at;
                b
            });
            self.cells.push((id, blocked));
            let picture = pictures.get(k).copied().flatten();
            if let (Some(p), Some(ws)) = (picture.or(fallback), &self.picture_seq) {
                self.pictures.push(Picture {
                    cell: id,
                    picture: p,
                    player: Player::new(ws, PICTURE),
                    hidden: picture.is_none(),
                });
            }
        }
        let fixed = self.texts.labels.clone();
        let mut fixed = fixed.into_iter();
        for spec in &LABELS {
            let text = match spec.message {
                Some(_) => fixed.next().unwrap_or_default(),
                None => Vec::new(),
            };
            self.labels.push(Label {
                props: spec.props,
                text,
                align: spec.align,
            });
        }
        self.labels[7].text = self.texts.story[0].clone();
        self.labels[8].text = self.texts.story[1].clone();
        // The episode panel waits in group 1 until the episodes open.
        let mut t = Text::new(self.texts.heading.clone(), {
            match seq.groups[PANEL[0]].text_position_x {
                0 => 15.,
                v => v as f32,
            }
        });
        t.align = 2;
        t.group_position = false;
        t.keep_rgb = false;
        let panel = self
            .sprites
            .create(seq, PRIO_PANEL, IMAGE, PANEL[3], None, Some(t));
        self.panel = Some(panel);
        let pitch = seq.groups[EPISODE_ROW].text_position_x as f32;
        let shown = state.episodes.len().min(landing::EPISODE_ROWS);
        for k in 0..shown {
            let row = self
                .sprites
                .create(seq, PRIO_EPISODE, IMAGE, EPISODE_ROW, None, None);
            self.sprites.attach(row, panel);
            self.sprites.get_mut(row).offset = [0., pitch * k as f32];
            let badge = self
                .sprites
                .create(seq, PRIO_BADGE, IMAGE, BADGE[0], None, None);
            self.sprites.attach(badge, row);
            self.episode_rows.push((row, badge));
        }
        self.show_episodes(seq, state);
    }

    /// FUN_1403928a0's row refresh: titles, help lines and NEW marks from
    /// the first shown episode.
    fn show_episodes(&mut self, seq: &Sequence, state: &Landing) {
        for (k, &(row, badge)) in self.episode_rows.clone().iter().enumerate() {
            let i = state.episode_top + k;
            let (title, help) = self.texts.episodes.get(i).cloned().unwrap_or_default();
            let size = text_size(seq, EPISODE_ROW);
            let s = self.sprites.get_mut(row);
            let mut t = Text::new(title, size);
            t.group_position = false;
            t.keep_rgb = false;
            s.text = Some(t);
            // FUN_140392680: the help line after the title [I: drawn as the
            // row's second text].
            let mut h = Text::new(help, size);
            h.offset = [seq.groups[EPISODE_ROW].light_position_x as f32, 0.];
            s.after = Some(h);
            let new = state.episodes.get(i).is_some_and(|e| e.new);
            self.sprites.set(seq, badge, BADGE[usize::from(new)], None);
        }
    }

    /// Applies the rules' changes (mode switches retarget the cells and
    /// labels, FUN_1403931f0; the notice and the close).
    pub fn apply(&mut self, layout: &Layout, state: &Landing, changes: &[Change]) {
        let seq = &layout.sequences[SEQUENCE];
        for &c in changes {
            match c {
                Change::Scene { scene, next } => {
                    self.task.set_scene(scene, Some(next));
                    self.mode = state.mode;
                    self.idle_reached = false;
                    if let Some(p) = self.panel {
                        if state.mode == landing::EPISODES {
                            self.sprites.set(seq, p, PANEL[0], Some(PANEL[1]));
                        } else {
                            self.sprites.set(seq, p, PANEL[2], Some(PANEL[3]));
                        }
                    }
                }
                Change::Notice(true) => {
                    let id = match self.notice.filter(|&n| self.sprites.alive(n)) {
                        Some(n) => {
                            self.sprites.set(seq, n, NOTICE[0], Some(NOTICE[1]));
                            n
                        }
                        None => {
                            let mut t =
                                Text::new(self.texts.notice.clone(), text_size(seq, NOTICE[0]));
                            t.align = 2;
                            t.group_position = false;
                            t.keep_rgb = false;
                            self.sprites.create(
                                seq,
                                PRIO_NOTICE,
                                IMAGE,
                                NOTICE[0],
                                Some(NOTICE[1]),
                                Some(t),
                            )
                        }
                    };
                    self.notice = Some(id);
                }
                Change::Notice(false) => {
                    if let Some(n) = self.notice {
                        self.sprites.set(seq, n, NOTICE[2], None);
                    }
                }
                Change::EpisodeTop(_) => self.show_episodes(seq, state),
                Change::Close => {
                    // FUN_140392430: the close scene; rows play their close
                    // group (FUN_1402f7db0); the marks and the cursor go.
                    self.task.set_scene(state.close_scene(), None);
                    for &r in &self.rows {
                        self.sprites.set(seq, r, ROW[1], None);
                    }
                    for &m in &self.marks {
                        self.sprites.kill(m);
                    }
                    self.marks.clear();
                    self.cursor.kill(&mut self.sprites);
                    if let Some(p) = self.panel
                        && state.mode == landing::EPISODES
                    {
                        self.sprites.set(seq, p, PANEL[2], None);
                    }
                }
            }
        }
    }

    /// Whether the layout plays the mode's idle scene (input runs then).
    pub fn ready(&self, state: &Landing) -> bool {
        !self.task.dead && self.task.scene == state.idle_scene()
    }

    /// The menu is gone (its close scene ended).
    pub fn done(&self) -> bool {
        self.task.dead
    }

    /// One frame: the layout, the sprites, the cells on their props, the
    /// selection groups and the cursor.
    pub fn update(&mut self, layout: &Layout, state: &Landing) {
        let seq = &layout.sequences[SEQUENCE];
        self.task.update(layout, 1.);
        let idle = !self.task.dead && self.task.scene == state.idle_scene();
        // FUN_140392010: on reaching the idle scene the rows / marks settle
        // and the cursor attaches.
        if idle && !self.idle_reached && !state.closing {
            self.idle_reached = true;
            self.last_cursor = None;
            if state.mode != landing::ROWS && self.marks.is_empty() {
                for g in MARKS {
                    let id = self.sprites.create(seq, PRIO_MARK, IMAGE, g, None, None);
                    self.marks.push(id);
                }
            }
        }
        if self.idle_reached && !state.closing && self.last_cursor != Some(state.cursor) {
            self.last_cursor = Some(state.cursor);
            for (k, &r) in self.rows.iter().enumerate() {
                let g = if k == state.cursor {
                    ROW_SELECTED
                } else {
                    ROW[2]
                };
                self.sprites.set(seq, r, g, None);
            }
            self.cursor.retarget();
        }
        // Cells (Tz::LaySeq) follow their prop for the scene.
        let props = CELL_PROPS[self.mode as usize];
        let slot = scene_slot(self.task.scene, self.mode);
        let cell_anchor = slot.and_then(|s| anchor(layout, &self.task, props[s]));
        for &(id, blocked) in &self.cells {
            for id in std::iter::once(id).chain(blocked) {
                let s = self.sprites.get_mut(id);
                match cell_anchor {
                    Some(a) => {
                        s.base = a.at;
                        s.multiplier[3] = a.color[3];
                        s.hidden = false;
                    }
                    None => s.hidden = true,
                }
            }
        }
        if let Some(ws) = &self.picture_seq {
            for p in &mut self.pictures {
                p.player.update(ws, 1.);
            }
        }
        self.sprites.update(seq);
        // The marks and the cursor sit on the selected cell or row.
        let target = if state.mode == landing::ROWS {
            self.rows.get(state.cursor).copied()
        } else {
            self.cells.get(state.cursor).map(|c| c.0)
        };
        if let Some(t) = target {
            // Children of the target (FUN_1402f6c10 / FUN_1402fb3b0): they
            // sit at its context.
            let at = self.sprites.context(t);
            for &m in &self.marks {
                self.sprites.get_mut(m).base = at;
            }
            if self.idle_reached && !state.closing {
                let x = if state.mode == landing::ROWS {
                    0.
                } else {
                    seq.groups[CELL].text_position_x as f32
                };
                self.cursor.place(&mut self.sprites, [at[0] + x, at[1]]);
            }
        }
    }

    pub fn draw(&self, layout: &Layout, state: &Landing, font: &Font) -> Vec<(Sheet, Vec<Quad>)> {
        let seq = &layout.sequences[SEQUENCE];
        let mut items: Vec<Item> = Vec::new();
        let m = Modifiers::default();
        if !self.task.dead {
            for (prop, player, origin) in self.task.player.visible(layout, [0, 0]) {
                items.push((
                    PRIO_LAYOUT,
                    Sheet::Image(prop.texture),
                    layout.sequences[prop.sequence].quads(player, origin, &m),
                ));
            }
        }
        // Tz::LayMsg texts on their props.
        let slot = scene_slot(self.task.scene, self.mode);
        let which = (self.mode as usize).saturating_sub(1).min(2);
        for (k, l) in self.labels.iter().enumerate() {
            let text = if k == 0 {
                self.texts
                    .cell_names
                    .get(state.cursor)
                    .cloned()
                    .unwrap_or_default()
            } else {
                l.text.clone()
            };
            let Some(a) = slot.and_then(|s| anchor(layout, &self.task, l.props[which][s])) else {
                continue;
            };
            if text.is_empty() {
                continue;
            }
            let g = &layout.sequences[a.sequence].groups[a.group];
            let size = match g.text_scale {
                s if s > 0 => s as f32,
                _ => 15.,
            };
            // FUN_140390d00 leaves the labels flag 4 only (the prop's
            // animation colour, no group offset); the two story texts get
            // flag 0x10 back (the group's LightPositionX / TextPositionY).
            let offset = if k >= 7 {
                [g.light_position_x as f32, g.text_position_y as f32]
            } else {
                [0.; 2]
            };
            let q = text_quads(
                &text,
                [a.at[0] + offset[0], a.at[1] + offset[1]],
                size,
                l.align,
                g.ui_padding as f32,
                a.color,
                true,
                font,
            );
            items.push((PRIO_TEXT, Sheet::Font, q.glyphs));
            if !q.icons.is_empty() {
                items.push((PRIO_TEXT, Sheet::Icon, q.icons));
            }
        }
        // The pictures on their cells.
        if let Some(ws) = &self.picture_seq {
            for p in &self.pictures {
                if p.hidden || !self.sprites.alive(p.cell) || self.sprites.get(p.cell).hidden {
                    continue;
                }
                let at = self.sprites.context(p.cell);
                let m = Modifiers {
                    color: [1., 1., 1., self.sprites.child_alpha(p.cell)],
                    ..Modifiers::default()
                };
                items.push((
                    PRIO_ROW,
                    Sheet::Picture(p.picture),
                    ws.quads(&p.player, at, &m),
                ));
            }
        }
        sprite_items(&self.sprites, seq, font, &mut items);
        ordered(items)
    }
}

/// Which of a mode's {open, close, idle} scenes `scene` is.
fn scene_slot(scene: usize, mode: u8) -> Option<usize> {
    landing::SCENES[mode as usize]
        .iter()
        .position(|&s| s == scene)
}
