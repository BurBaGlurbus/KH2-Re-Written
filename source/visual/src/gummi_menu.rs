//! The Gummi menu's 2D screens on `gumimenu/<lang>/gumimenu.bar` `0000`
//! ("gummi_menu_lay"): its scenes (the frame and title, `0004`'s
//! transitions) and its items (`gumimenu/menuentry.bar` records, gm::MENUITEM,
//! FUN_140270e80). An item plays its record's appear group, then its loop;
//! selected, the record's selected groups. Its text is drawn at the loop
//! group's IS_ACTIVE anchor with that anchor's colour and the group's text
//! size (FUN_140271900). [Not built] The 3D room and panels (G_MN100_BG,
//! G_MN200_*): their camera and easing aren't traced, so the screen is drawn
//! over black.
use crate::field_command_menu::Font;
use crate::sprite_player::{self, Id, Sprites, Text};
use crate::world_map_menu::Sheet;
use data::gummi_table::{MenuEntry, Transition};
use data::menu_layout::{Layout, LayoutTask};
use data::sprite_sequence::{Modifiers, Quad};
use gummi::menu::{self as rules, Step, TOP_CANCEL_ROW, TOP_ROWS, TopChoice};
use std::collections::HashMap;

/// The data the screens play from.
pub struct GummiData {
    pub entries: Vec<MenuEntry>,
    pub transitions: Vec<Transition>,
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
/// The course name banner on a fresh top menu (COURSENAME_TOP).
const BANNER: usize = 0x1c4;

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
    /// FUN_140271210: appear, then loop; `selected` uses the selected
    /// groups, and `via` plays the normal appear first.
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
        let groups = data.entries[skin.unwrap_or(entry)].clone();
        let seq_index = e.sequence.max(0) as usize;
        let seq = &layout.sequences[seq_index];
        if self.sprites.len() <= seq_index {
            self.sprites.resize_with(seq_index + 1, Sprites::default);
        }
        let g = [groups.normal, groups.selected];
        let (first, queued, after) = if selected {
            // The cursor row: appear, selected appear, selected loop
            // (FUN_140274b00).
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
        let id = self.sprites[seq_index].create(
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

    /// FUN_140271310: the close group, and the item ends with it.
    fn close(&mut self, layout: &Layout, k: usize) {
        let item = &mut self.list[k];
        item.after = None;
        match group(item.groups[usize::from(item.selected)][2]) {
            Some(g) => self.sprites[item.seq].close(&layout.sequences[item.seq], item.id, g),
            None => self.sprites[item.seq].kill(item.id),
        }
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

/// What the top menu asks of its host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TopEvent {
    Sound(u32),
    /// A row was decided; the screen is closing towards it.
    Chosen(TopChoice),
}

/// Screen 0 (FUN_14024e780 enter, FUN_14024e4e0 update, FUN_14024e470 close).
pub struct TopMenu {
    layout: LayoutTask,
    items: Items,
    rows: [usize; 3],
    help: usize,
    banner: usize,
    pub cursor: usize,
    pub chosen: Option<TopChoice>,
    level1_cleared: bool,
    blueprints: bool,
}

impl TopMenu {
    /// A fresh top menu ("none" → "top") for `route` 0..=8 named
    /// `route_name`.
    pub fn new(
        layout: &Layout,
        data: &GummiData,
        route_name: Vec<u8>,
        cursor: usize,
        level1_cleared: bool,
        blueprints: bool,
    ) -> Self {
        let (first, next) = data.scenes("none", "top").unwrap_or((34, 35));
        let mut task = LayoutTask::new(layout, first);
        task.set_scene(first, Some(next));
        let mut items = Items::default();
        let rows = std::array::from_fn(|k| {
            let (entry, _) = TOP_ROWS[k];
            let text = data.text(data.entries[entry].text);
            items.open(layout, data, entry, None, text, k == cursor)
        });
        let banner = items.open(layout, data, BANNER, None, route_name, false);
        let help = items.open(
            layout,
            data,
            HELP,
            Some(HELP_SKIN),
            data.text(TOP_ROWS[cursor].1 as i32),
            false,
        );
        Self {
            layout: task,
            items,
            rows,
            help,
            banner,
            cursor,
            chosen: None,
            level1_cleared,
            blueprints,
        }
    }

    fn move_to(&mut self, layout: &Layout, data: &GummiData, row: usize, from: usize) {
        self.items.select(layout, self.rows[from], false);
        self.items.select(layout, self.rows[row], true);
        self.items
            .set_text(self.help, data.text(TOP_ROWS[row].1 as i32));
    }

    /// One frame of input and play.
    pub fn tick(
        &mut self,
        layout: &Layout,
        data: &GummiData,
        pressed: u64,
        repeat: u64,
    ) -> Vec<TopEvent> {
        let mut out = Vec::new();
        if self.chosen.is_none() {
            let before = self.cursor;
            match rules::selector(&mut self.cursor, 3, pressed, repeat) {
                Step::Moved => {
                    out.push(TopEvent::Sound(rules::sound::MOVE));
                    self.move_to(layout, data, self.cursor, before);
                }
                Step::Cancel => {
                    out.push(TopEvent::Sound(rules::sound::CANCEL));
                    if self.cursor != TOP_CANCEL_ROW {
                        self.cursor = TOP_CANCEL_ROW;
                        out.push(TopEvent::Sound(rules::sound::MOVE));
                        self.move_to(layout, data, TOP_CANCEL_ROW, before);
                    }
                }
                Step::Decide(row) => {
                    out.push(TopEvent::Sound(rules::sound::DECIDE));
                    let choice = rules::top_choice(row, self.level1_cleared, self.blueprints);
                    if row == 0 {
                        out.push(TopEvent::Sound(rules::sound::MISSION));
                    }
                    self.chosen = Some(choice);
                    for k in self.rows {
                        self.items.close(layout, k);
                    }
                    // Leaving for anything but mission select closes the
                    // banner and the help window too.
                    if choice != TopChoice::Mission {
                        self.items.close(layout, self.banner);
                        self.items.close(layout, self.help);
                    }
                    out.push(TopEvent::Chosen(choice));
                }
                Step::None => {}
            }
        }
        self.layout.update(layout, 1.);
        self.items.update(layout);
        out
    }

    /// The rows have finished closing.
    pub fn closed(&self) -> bool {
        self.chosen.is_some() && self.rows.iter().all(|&k| !self.items.alive(k))
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
        self.items.list.iter().map(|i| i.entry).collect()
    }
}
