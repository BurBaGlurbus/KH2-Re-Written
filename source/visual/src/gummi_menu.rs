//! The Gummi menu's 2D screens on `gumimenu/<lang>/gumimenu.bar` `0000`
//! ("gummi_menu_lay"): its scenes (the frame and title, `0004`'s
//! transitions) and its items (`gumimenu/menuentry.bar` records, gm::MENUITEM,
//! FUN_140270e80). An item plays its record's appear group, then its loop;
//! selected, the record's selected groups. Its text is drawn at the loop
//! group's IS_ACTIVE anchor with that anchor's colour and the group's text
//! size (FUN_140271900). The help window and the course banner stay up from
//! screen to screen. [Not built] The 3D room, rows and carousel (G_MN100_BG,
//! G_MN200_*): their camera and easing aren't traced, so the screens are
//! drawn over black; the EX prompt's button glyph.
use crate::field_command_menu::Font;
use crate::sprite_player::{self, Id, Sprites, Text};
use crate::world_map_menu::Sheet;
use data::gummi_table::{DigitStyle, MenuEntry, Transition};
use data::menu_layout::{Layout, LayoutTask};
use data::sprite_sequence::{Modifiers, Quad};
use gummi::Mission;
use gummi::menu::{
    self as rules, LEVEL_PANELS, Step, TOP_CANCEL_ROW, TOP_ROWS, TopChoice, mission_pad,
    mission_sound,
};
use std::collections::HashMap;

/// The data the screens play from.
pub struct GummiData {
    pub entries: Vec<MenuEntry>,
    pub transitions: Vec<Transition>,
    pub digits: Vec<DigitStyle>,
    /// `gumibattle/treasure.bin` rows as (route, level byte, obtained bit).
    pub treasures: Vec<(u8, u8, u32)>,
    /// `msg/<lang>/gumi.bar` by id.
    pub texts: HashMap<u32, Vec<u8>>,
}

impl GummiData {
    fn text(&self, id: i32) -> Vec<u8> {
        u32::try_from(id)
            .ok()
            .and_then(|id| self.texts.get(&id))
            .cloned()
            .unwrap_or_default()
    }

    /// FUN_140252e50: the scenes going from screen `from` to `to`.
    pub fn scenes(&self, from: &str, to: &str) -> Option<(usize, usize)> {
        let t = self
            .transitions
            .iter()
            .find(|t| t.layout == "gummi_menu_lay" && t.from == from && t.to == to)?;
        let first = *t.scenes.first()? as usize;
        Some((first, *t.scenes.last()? as usize))
    }
}

/// The help window: record 0xf, with 0x15a's orange groups
/// (MENU_HELP_COLOR_ORANGE, FUN_140271710).
const HELP: usize = 0xf;
const HELP_SKIN: usize = 0x15a;
/// The course name banner: fresh on the top menu, then its moves between
/// the top menu and mission select (FUN_140276640).
const BANNER_TOP: usize = 0x1c4;
const BANNER_TO_MISSION: usize = 0x1c6;
const BANNER_TO_TOP: usize = 0x1c7;
/// Mission select's help text.
const MISSION_HELP: i32 = 0x1904;
/// Mission select's counters and marks (FUN_140277e30).
const RANK: usize = 0x155;
const TREASURE: usize = 0x157;
const PERCENT: usize = 0x1c0;
const EX_BADGE: usize = 0x262;
const EX_PROMPT: usize = 0x263;
/// The prompt's texts: "EX Mission" while EX is off, "Normal Mission" on.
const EX_TEXTS: [i32; 2] = [0x5014, 0x5015];
/// The description's text changes this long into a level slide
/// (DAT_140623bb4).
const TEXT_DELAY: u32 = 10;

/// One menu item: its sprite on its record's sequence, and the group to
/// queue once the queued one has started.
struct Item {
    entry: usize,
    seq: usize,
    id: Id,
    after: Option<usize>,
    groups: [[i32; 3]; 2],
    selected: bool,
}

fn group(g: i32) -> Option<usize> {
    usize::try_from(g).ok()
}

/// Items on one layout, kept per sequence (`Sprites` plays one).
#[derive(Default)]
struct Items {
    sprites: Vec<Sprites>,
    list: Vec<Item>,
}

impl Items {
    fn sprites(&mut self, seq: usize) -> &mut Sprites {
        if self.sprites.len() <= seq {
            self.sprites.resize_with(seq + 1, Sprites::default);
        }
        &mut self.sprites[seq]
    }

    /// FUN_140271210: appear, then loop; `selected` uses the selected
    /// groups after the plain appear (FUN_140274b00).
    fn open(
        &mut self,
        layout: &Layout,
        data: &GummiData,
        entry: usize,
        skin: Option<usize>,
        text: Vec<u8>,
        selected: bool,
    ) -> usize {
        let e = &data.entries[entry];
        let skin = &data.entries[skin.unwrap_or(entry)];
        let seq_index = e.sequence.max(0) as usize;
        let seq = &layout.sequences[seq_index];
        let g = [skin.normal, skin.selected];
        let (first, queued, after) = if selected {
            (g[0][0], g[1][0], group(g[1][1]))
        } else {
            (g[0][0], g[0][1], None)
        };
        let first = group(first).or(group(queued)).unwrap_or(0);
        let size = group(g[0][1])
            .and_then(|k| seq.groups.get(k))
            .map_or(14., |k| k.text_scale as f32);
        let mut t = Text::new(text, size);
        t.align = 2;
        t.group_position = false;
        t.keep_rgb = false;
        let id = self.sprites(seq_index).create(
            seq,
            1,
            e.texture.max(0) as usize,
            first,
            group(queued).filter(|&q| q != first),
            Some(t),
        );
        self.list.push(Item {
            entry,
            seq: seq_index,
            id,
            after,
            groups: g,
            selected,
        });
        self.list.len() - 1
    }

    /// A digit of a counter (FUN_1402777b0), attached to `parent` at `x`.
    fn digit(&mut self, layout: &Layout, style: &DigitStyle, d: u32, parent: usize, x: f32) {
        let seq_index = style.sequence.max(0) as usize;
        let p = &self.list[parent];
        let (pseq, pid) = (p.seq, p.id);
        let Some(g) = group(style.digits[d as usize % 10].into()) else {
            return;
        };
        let seq = &layout.sequences[seq_index];
        let id =
            self.sprites(seq_index)
                .create(seq, 0x5e, style.texture.max(0) as usize, g, None, None);
        // A digit follows its item's position and alpha.
        if pseq == seq_index {
            let s = &mut self.sprites[seq_index];
            s.attach(id, pid);
            let d = s.get_mut(id);
            d.offset = [x, 0.];
            d.inherit_alpha = true;
            d.die_with_parent = true;
        }
        self.list.push(Item {
            entry: usize::MAX,
            seq: seq_index,
            id,
            after: None,
            groups: [[-1; 3]; 2],
            selected: false,
        });
    }

    /// A counter of `slots` digits right to left from its width, leading
    /// zeros hidden (FUN_14026aed0).
    fn counter(
        &mut self,
        layout: &Layout,
        style: &DigitStyle,
        slots: usize,
        value: u32,
        parent: usize,
    ) {
        let seq = &layout.sequences[style.sequence.max(0) as usize];
        let advance = |d: u32| {
            group(style.digits[d as usize].into())
                .and_then(|g| seq.groups.get(g))
                .map_or(0., |g| g.ui_padding as f32)
        };
        let mut x = advance(0) * slots as f32;
        let mut v = value;
        for k in 0..slots {
            if k > 0 && v == 0 {
                break;
            }
            let d = v % 10;
            x -= advance(d);
            self.digit(layout, style, d, parent, x);
            v /= 10;
        }
    }

    /// Puts item `k` on `parent`'s sprite at `x`, following its position
    /// and alpha (same sequence only).
    fn attach(&mut self, k: usize, parent: usize, x: f32) {
        let (c, p) = (&self.list[k], &self.list[parent]);
        if c.seq != p.seq {
            return;
        }
        let (seq, cid, pid) = (c.seq, c.id, p.id);
        let s = &mut self.sprites[seq];
        s.attach(cid, pid);
        let d = s.get_mut(cid);
        d.offset = [x, 0.];
        d.inherit_alpha = true;
    }

    /// FUN_140270d60: straight to the selected or plain loop.
    fn select(&mut self, layout: &Layout, k: usize, on: bool) {
        let item = &mut self.list[k];
        if item.selected == on {
            return;
        }
        item.selected = on;
        item.after = None;
        if let Some(g) = group(item.groups[usize::from(on)][1]) {
            self.sprites[item.seq].set(&layout.sequences[item.seq], item.id, g, None);
        }
    }

    /// FUN_140271710 then the record's appear: another record's groups, and
    /// `then`'s loop after them.
    fn reskin(&mut self, layout: &Layout, data: &GummiData, k: usize, entry: usize, then: usize) {
        let skin = &data.entries[entry];
        let next = &data.entries[then];
        let item = &mut self.list[k];
        item.groups = [next.normal, next.selected];
        item.after = None;
        let seq = &layout.sequences[item.seq];
        if let Some(g) = group(skin.normal[0]) {
            self.sprites[item.seq].set(seq, item.id, g, group(next.normal[1]));
        }
    }

    /// FUN_140271310: the close group, and the item ends with it.
    fn close(&mut self, layout: &Layout, k: usize) {
        let item = &mut self.list[k];
        item.after = None;
        match group(item.groups[usize::from(item.selected)][2]) {
            Some(g) => self.sprites[item.seq].close(&layout.sequences[item.seq], item.id, g),
            None => self.sprites[item.seq].kill(item.id),
        }
    }

    fn kill(&mut self, k: usize) {
        let item = &self.list[k];
        self.sprites[item.seq].kill(item.id);
    }

    fn set_text(&mut self, k: usize, text: Vec<u8>) {
        let item = &self.list[k];
        if let Some(t) = self.sprites[item.seq].get_mut(item.id).text.as_mut() {
            t.bytes = text;
        }
    }

    fn alive(&self, k: usize) -> bool {
        let item = &self.list[k];
        self.sprites[item.seq].alive(item.id)
    }

    fn update(&mut self, layout: &Layout) {
        for item in &mut self.list {
            let s = &mut self.sprites[item.seq];
            if s.alive(item.id) && s.get(item.id).queued.is_none() {
                s.get_mut(item.id).queued = item.after.take();
            }
        }
        for (k, s) in self.sprites.iter_mut().enumerate() {
            if let Some(seq) = layout.sequences.get(k) {
                s.update(seq);
            }
        }
    }

    fn draw(&self, layout: &Layout, font: &Font) -> Vec<(Sheet, Vec<Quad>)> {
        let mut out = Vec::new();
        for (k, s) in self.sprites.iter().enumerate() {
            for (sheet, quads) in s.draw(&layout.sequences[k], font) {
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

/// Where the menu ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Top(TopChoice),
    /// Mission select decided: level 1..=3, EX or not (ship select next).
    Fly {
        level: u8,
        ex: bool,
    },
}

/// What the menu asks of its host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuEvent {
    Sound(u32),
    /// A screen began ("top", "mission").
    Screen(&'static str),
    /// The menu is closing towards this.
    Chosen(Choice),
}

/// Screen 1's state (FUN_14024ee90).
struct MissionScreen {
    index: usize,
    levels: usize,
    ex: bool,
    panel: usize,
    /// The counters, labels and EX marks rebuilt on each change.
    info: Vec<usize>,
    /// Input waits for the carousel's turn.
    wait: u32,
    /// The description's text, after the slide's delay.
    swap: Option<(u32, Vec<u8>)>,
}

enum Screen {
    Top { rows: [usize; 3] },
    Mission(MissionScreen),
}

/// The Gummi menu mode's screens 0 and 1 (FUN_14024e780 / FUN_14024e4e0
/// top menu, FUN_14024ee90 / FUN_14024e9e0 mission select).
pub struct GummiMenu {
    layout: LayoutTask,
    items: Items,
    help: usize,
    banner: usize,
    screen: Screen,
    /// The top menu's cursor (its screen stack entry keeps it).
    pub cursor: usize,
    pub chosen: Option<Choice>,
    /// The route's normal and EX records, `route` 0..=8.
    records: [[Mission; 3]; 2],
    route: u8,
    blueprints: bool,
}

impl GummiMenu {
    /// The menu mode's start ("none" → "top") for `route` 0..=8 named
    /// `route_name`.
    pub fn new(
        layout: &Layout,
        data: &GummiData,
        route: u8,
        route_name: Vec<u8>,
        records: [[Mission; 3]; 2],
        blueprints: bool,
    ) -> Self {
        let (first, next) = data.scenes("none", "top").unwrap_or((34, 35));
        let mut task = LayoutTask::new(layout, first);
        task.set_scene(first, Some(next));
        let mut items = Items::default();
        let banner = items.open(layout, data, BANNER_TOP, None, route_name, false);
        let help = items.open(layout, data, HELP, Some(HELP_SKIN), Vec::new(), false);
        let mut menu = Self {
            layout: task,
            items,
            help,
            banner,
            screen: Screen::Top { rows: [0; 3] },
            cursor: 0,
            chosen: None,
            records,
            route,
            blueprints,
        };
        menu.open_top(layout, data);
        menu
    }

    fn open_top(&mut self, layout: &Layout, data: &GummiData) {
        let cursor = self.cursor;
        let rows = std::array::from_fn(|k| {
            let (entry, _) = TOP_ROWS[k];
            let text = data.text(data.entries[entry].text);
            self.items
                .open(layout, data, entry, None, text, k == cursor)
        });
        self.items
            .set_text(self.help, data.text(TOP_ROWS[cursor].1 as i32));
        self.screen = Screen::Top { rows };
    }

    fn open_mission(&mut self, layout: &Layout, data: &GummiData) {
        let levels = rules::level_count(&self.records[0]);
        let text = data.text(rules::level_text(self.route, 0, false) as i32);
        let panel = self
            .items
            .open(layout, data, LEVEL_PANELS[0], None, text, false);
        self.items.set_text(self.help, data.text(MISSION_HELP));
        let mut m = MissionScreen {
            index: 0,
            levels,
            ex: false,
            panel,
            info: Vec::new(),
            wait: 0,
            swap: None,
        };
        self.fill_info(layout, data, &mut m);
        self.screen = Screen::Mission(m);
    }

    /// The level's rank, value, treasure share and EX marks (FUN_140277e30).
    fn fill_info(&mut self, layout: &Layout, data: &GummiData, m: &mut MissionScreen) {
        for k in m.info.drain(..) {
            self.items.kill(k);
        }
        let start = self.items.list.len();
        let record = self.records[usize::from(m.ex)][m.index];
        let style = |s: usize| data.digits.get(s);
        let rank = self.items.open(layout, data, RANK, None, Vec::new(), false);
        let (s, d) = rules::rank_digit(record.rank);
        if let Some(st) = style(s) {
            self.items.counter(layout, st, 1, d, rank);
        }
        let (entry, s, slots, value) = rules::level_value(m.index, &record);
        let shown = self
            .items
            .open(layout, data, entry, None, Vec::new(), false);
        if let Some(st) = style(s) {
            self.items.counter(layout, st, slots, value, shown);
        }
        let share = self
            .items
            .open(layout, data, TREASURE, None, Vec::new(), false);
        if let Some(st) = style(4) {
            self.items
                .counter(layout, st, 3, record.treasure as u32, share);
        }
        // "%" after the three digits (3 × the style's width plus its own
        // group's +0x1c), on the treasure item.
        let percent = self
            .items
            .open(layout, data, PERCENT, None, Vec::new(), false);
        if let Some(st) = style(4) {
            let seq = &layout.sequences[st.sequence.max(0) as usize];
            let width = |g: Option<usize>| {
                g.and_then(|g| seq.groups.get(g))
                    .map_or(0., |g| g.ui_padding as f32)
            };
            let own = width(group(data.entries[PERCENT].normal[1]));
            let x = 3. * width(group(st.digits[0].into())) + own;
            self.items.attach(percent, share, x);
        }
        if m.ex {
            self.items
                .open(layout, data, EX_BADGE, None, Vec::new(), false);
        }
        if rules::ex_available(&self.records[1], m.index) {
            let prompt = data.text(EX_TEXTS[usize::from(m.ex)]);
            self.items
                .open(layout, data, EX_PROMPT, None, prompt, false);
        }
        m.info = (start..self.items.list.len()).collect();
    }

    fn play_transition(&mut self, data: &GummiData, from: &str, to: &str) {
        if let Some((first, next)) = data.scenes(from, to) {
            self.layout.set_scene(first, Some(next));
        }
    }

    fn top_tick(
        &mut self,
        layout: &Layout,
        data: &GummiData,
        rows: [usize; 3],
        pressed: u64,
        repeat: u64,
        out: &mut Vec<MenuEvent>,
    ) {
        let before = self.cursor;
        let moved = |menu: &mut Self, row: usize| {
            menu.items.select(layout, rows[before], false);
            menu.items.select(layout, rows[row], true);
            menu.items
                .set_text(menu.help, data.text(TOP_ROWS[row].1 as i32));
        };
        match rules::selector(&mut self.cursor, 3, pressed, repeat) {
            Step::Moved => {
                out.push(MenuEvent::Sound(rules::sound::MOVE));
                moved(self, self.cursor);
            }
            Step::Cancel => {
                out.push(MenuEvent::Sound(rules::sound::CANCEL));
                if self.cursor != TOP_CANCEL_ROW {
                    self.cursor = TOP_CANCEL_ROW;
                    out.push(MenuEvent::Sound(rules::sound::MOVE));
                    moved(self, TOP_CANCEL_ROW);
                }
            }
            Step::Decide(row) => {
                out.push(MenuEvent::Sound(rules::sound::DECIDE));
                let cleared = self.records[0][0].cleared;
                let choice = rules::top_choice(row, cleared, self.blueprints);
                for k in rows {
                    self.items.close(layout, k);
                }
                if choice == TopChoice::Mission {
                    out.push(MenuEvent::Sound(rules::sound::MISSION));
                    out.push(MenuEvent::Sound(mission_sound::OPEN));
                    out.push(MenuEvent::Screen("mission"));
                    self.items.reskin(
                        layout,
                        data,
                        self.banner,
                        BANNER_TO_MISSION,
                        BANNER_TO_MISSION,
                    );
                    self.play_transition(data, "top", "mission");
                    self.open_mission(layout, data);
                } else {
                    if row == 0 {
                        out.push(MenuEvent::Sound(rules::sound::MISSION));
                    }
                    self.items.close(layout, self.banner);
                    self.items.close(layout, self.help);
                    self.chosen = Some(Choice::Top(choice));
                    out.push(MenuEvent::Chosen(Choice::Top(choice)));
                }
            }
            Step::None => {}
        }
    }

    fn mission_tick(
        &mut self,
        layout: &Layout,
        data: &GummiData,
        mut m: MissionScreen,
        pressed: u64,
        repeat: u64,
        out: &mut Vec<MenuEvent>,
    ) -> Option<MissionScreen> {
        if let Some((n, text)) = m.swap.as_mut() {
            *n = n.saturating_sub(1);
            if *n == 0 {
                let text = std::mem::take(text);
                self.items.set_text(m.panel, text);
                m.swap = None;
            }
        }
        if m.wait > 0 {
            m.wait -= 1;
            return Some(m);
        }
        let close_info = |menu: &mut Self, m: &MissionScreen| {
            menu.items.close(layout, m.panel);
            for &k in &m.info {
                menu.items.close(layout, k);
            }
        };
        if pressed & rules::pad::DECIDE != 0 {
            out.push(MenuEvent::Sound(rules::sound::DECIDE));
            close_info(self, &m);
            self.items.close(layout, self.banner);
            self.items.close(layout, self.help);
            let choice = Choice::Fly {
                level: m.index as u8 + 1,
                ex: m.ex,
            };
            self.chosen = Some(choice);
            out.push(MenuEvent::Chosen(choice));
            return Some(m);
        }
        if pressed & rules::pad::CANCEL != 0 {
            out.push(MenuEvent::Sound(rules::sound::CANCEL));
            out.push(MenuEvent::Sound(rules::sound::MISSION));
            close_info(self, &m);
            self.items
                .reskin(layout, data, self.banner, BANNER_TO_TOP, BANNER_TO_TOP);
            self.play_transition(data, "mission", "top");
            out.push(MenuEvent::Screen("top"));
            self.open_top(layout, data);
            return None;
        }
        if pressed & mission_pad::EX != 0 && rules::ex_available(&self.records[1], m.index) {
            out.push(MenuEvent::Sound(mission_sound::EX));
            m.ex = !m.ex;
            let text = data.text(rules::level_text(self.route, m.index, m.ex) as i32);
            self.items.set_text(m.panel, text);
            self.fill_info(layout, data, &mut m);
            return Some(m);
        }
        let step = if repeat & mission_pad::NEXT != 0 {
            Some(true)
        } else if repeat & mission_pad::PREVIOUS != 0 {
            Some(false)
        } else {
            None
        };
        if let Some(next) = step {
            let from = m.index;
            m.index = if next {
                (from + 1) % m.levels
            } else {
                (from + m.levels - 1) % m.levels
            };
            out.push(MenuEvent::Sound(if next {
                mission_sound::NEXT
            } else {
                mission_sound::PREVIOUS
            }));
            out.push(MenuEvent::Sound(rules::sound::MOVE));
            if !rules::ex_available(&self.records[1], m.index) {
                m.ex = false;
            }
            if let Some(slide) = rules::level_slide(from, m.index, next) {
                self.items
                    .reskin(layout, data, m.panel, slide, LEVEL_PANELS[m.index]);
            }
            let text = data.text(rules::level_text(self.route, m.index, m.ex) as i32);
            m.swap = Some((TEXT_DELAY, text));
            m.wait = rules::turn_frames(m.levels);
            self.fill_info(layout, data, &mut m);
        }
        Some(m)
    }

    /// One frame of input and play.
    pub fn tick(
        &mut self,
        layout: &Layout,
        data: &GummiData,
        pressed: u64,
        repeat: u64,
    ) -> Vec<MenuEvent> {
        let mut out = Vec::new();
        if self.chosen.is_none() {
            match std::mem::replace(&mut self.screen, Screen::Top { rows: [0; 3] }) {
                Screen::Top { rows } => {
                    self.screen = Screen::Top { rows };
                    self.top_tick(layout, data, rows, pressed, repeat, &mut out);
                }
                Screen::Mission(m) => {
                    if let Some(m) = self.mission_tick(layout, data, m, pressed, repeat, &mut out) {
                        self.screen = Screen::Mission(m);
                    }
                }
            }
        }
        self.layout.update(layout, 1.);
        self.items.update(layout);
        out
    }

    /// Everything has finished closing.
    pub fn closed(&self) -> bool {
        self.chosen.is_some() && (0..self.items.list.len()).all(|k| !self.items.alive(k))
    }

    pub fn draw(&self, layout: &Layout, font: &Font) -> Vec<(Sheet, Vec<Quad>)> {
        let m = Modifiers::default();
        let mut out = Vec::new();
        if !self.layout.dead {
            for (prop, player, origin) in self.layout.player.visible(layout, [0, 0]) {
                out.push((
                    Sheet::Image(prop.texture),
                    layout.sequences[prop.sequence].quads(player, origin, &m),
                ));
            }
        }
        out.extend(self.items.draw(layout, font));
        out
    }

    /// The `menuentry.bar` records on screen (for logs).
    pub fn items(&self) -> Vec<usize> {
        (0..self.items.list.len())
            .filter(|&k| self.items.alive(k) && self.items.list[k].entry != usize::MAX)
            .map(|k| self.items.list[k].entry)
            .collect()
    }
}
