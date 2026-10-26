//! The file menu's screens (`menu/<lang>/save.2ld`): the save point's rows,
//! the four-row file cards, the question and message windows. The rules are
//! `save::menu`; this draws them with the 2LD's sequences. Sequence 0 holds
//! the frame, tab, rows, windows and cursor, sequence 1 the eight card
//! pictures, sequence 2 the world icons (survey S6; tables at 0x1405ba3d0 ..
//! 0x1405ba670 and 0x1405bbb68 .. 0x1405bbbd8).

use crate::{
    menu_screen::{self, Menu2ld},
    pause_menu::encode_number,
    sprite_player::{Id, Sheet, Sprites, Text},
};
use data::{
    menu_layout::LayoutTask,
    sprite_sequence::{Quad, Sequence},
};
use save::{
    menu::{Backend, Dialog, Event, Input, Menu, Purpose, ROWS, message},
    slots::{SLOT_COUNT, Slot, Summary},
};
use std::collections::HashMap;

/// sys.bar message ids the screens need (`& 0x7fff`), beyond
/// `save::menu::message`'s own.
pub fn message_ids() -> Vec<u32> {
    use message as m;
    let mut ids = vec![
        m::SAVE,
        m::LOAD,
        m::WORLD_MAP,
        m::TITLE_SCREEN,
        m::JUMP_QUESTION,
        m::RETURN_TO_TITLE_HELP,
        m::SAVE_HELP,
        m::WORLD_MAP_HELP,
        m::EXIT_QUESTION,
        m::FAILED_SAVE,
        m::FAILED_CHECK,
        m::FAILED_LOAD,
        m::DELETING_PREVIOUS,
        m::SAVING,
        m::LOADING,
        m::DELETING,
        m::FAILED_DELETE,
        m::CORRUPT,
        m::SAVE_NEW_FILE,
        m::NO_SPACE,
        m::NO_DATA,
        m::HELP_SAVE_LIST,
        m::HELP_LOAD_LIST,
        m::OVERWRITE_QUESTION,
        m::DELETE_QUESTION,
        m::FILE_SYSTEM_CORRUPT,
        m::NO,
        m::YES,
        m::OK,
        m::LEVEL,
        m::MUNNY,
    ];
    ids.extend(m::DIFFICULTY.iter().flatten());
    ids
}

/// Message texts by id, and place names by (world, area).
#[derive(Clone, Debug, Default)]
pub struct Texts {
    pub messages: HashMap<u32, Vec<u8>>,
    pub places: HashMap<(u8, u8), Vec<u8>>,
    /// The "???" shown for a place with no name.
    pub unknown_place: Vec<u8>,
}

impl Texts {
    pub fn message(&self, id: u32) -> Vec<u8> {
        self.messages
            .get(&(id & 0x7fff))
            .cloned()
            .unwrap_or_default()
    }
}

/// Layout scene numbers of save.2ld's LAYD `save`: open, idle, close.
const SCENE: [usize; 3] = [0, 1, 2];

/// Priorities: frame < tab/help < rows/cards < dialog.
const PRIO_TAB: u32 = 0x7558;
const PRIO_ROWS: u32 = 0x755f;
const PRIO_CARD_ART: u32 = 0x7560;
const PRIO_CARD_TEXT: u32 = 0x7561;
const PRIO_DIALOG: u32 = 0x7570;

/// Sequence 0 groups: tab (in, out, idle), top rows (appear, close, idle,
/// selected), file cards, question and notice rows.
const TAB: [usize; 3] = [52, 54, 53];
const TOP_ROW: [usize; 4] = [79, 81, 80, 78];
const CARD: [usize; 4] = [86, 88, 87, 85];
const CHOICE: [usize; 4] = [25, 27, 26, 24];
/// The message window (open, idle, close).
const WINDOW: [usize; 3] = [4, 5, 6];
/// Where the rows and cards sit.
const TOP_PITCH: f32 = 34.;
const CARD_PITCH: f32 = 72.;
/// The card group's own translation (its y, 0x1405ba..: A213 T=(244,106)).
const CARD_ORIGIN_Y: f32 = 106.;
const TEXT_SIZE: f32 = 15.;
/// Centre of the message window's box (its parts span -191..194 and
/// -91..95 around the group's translation, 256, 192).
const DIALOG_AT: [f32; 2] = [257.5, 194.];
/// The size the font's widths are given for (`sprite_player`).
const FONT_REFERENCE: f32 = 18.;
const CARD_TEXT_SIZE: f32 = 13.;
/// Card text slots (x, y, align) from table 0x1405bbbd0: level, munny,
/// difficulty, place, time, message.
const CARD_TEXT: [(f32, f32, u8); 6] = [
    (120., -14., 0),
    (300., -14., 1),
    (420., -14., 1),
    (120., 14., 0),
    (420., 14., 1),
    (120., 0., 0),
];
/// Card world icon group by world index (0x1405bbb68) and card picture
/// group by the saved background byte (0x1405bbb90).
const WORLD_ICON: [usize; 19] = [
    20, 7, 17, 5, 8, 2, 10, 1, 13, 15, 11, 12, 4, 18, 14, 19, 3, 16, 6,
];
const BACKGROUND: [usize; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
/// Icon group a world with the "final" flag shows.
const FINAL_ICON: usize = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shown {
    None,
    Top,
    List(Purpose),
}

pub struct FileMenu {
    pub menu: Menu,
    texts: Texts,
    frame: LayoutTask,
    closing: bool,
    pub finished: bool,
    s0: Sprites,
    s1: Sprites,
    s2: Sprites,
    /// What the sprites below currently show.
    shown: Shown,
    dialog_key: Option<(u32, u8)>,
    tab: Option<Id>,
    top_rows: Vec<Id>,
    top_cursor: usize,
    cards: Vec<Card>,
    list_start: usize,
    list_revision: u32,
    dialog: Vec<Id>,
    dialog_rows: Vec<Id>,
    dialog_text: Vec<Id>,
    /// The message window box (sequence 0 copy, refitted per message) and
    /// its text, drawn over the cards.
    s3: Sprites,
    s4: Sprites,
    dialog_seq: Sequence,
    widths: Vec<u8>,
    help: Option<Id>,
    frames: u32,
}

struct Card {
    row: Id,
    picture: Id,
    icon: Id,
    texts: Vec<Id>,
}

/// What a host needs to draw the sprites.
pub struct Draw {
    pub art: Vec<(usize, Vec<Quad>)>,
    pub sheets: Vec<(Sheet, Vec<Quad>)>,
}

impl FileMenu {
    pub fn new(
        menu: Menu,
        texts: Texts,
        layout: &data::menu_layout::Layout,
        widths: Vec<u8>,
    ) -> Self {
        let mut frame = LayoutTask::new(layout, SCENE[0]);
        frame.next = Some(SCENE[1]);
        Self {
            menu,
            texts,
            frame,
            closing: false,
            finished: false,
            s0: Sprites::default(),
            s1: Sprites::default(),
            s2: Sprites::default(),
            shown: Shown::None,
            dialog_key: None,
            tab: None,
            top_rows: Vec::new(),
            top_cursor: usize::MAX,
            cards: Vec::new(),
            list_start: usize::MAX,
            list_revision: 0,
            dialog: Vec::new(),
            dialog_rows: Vec::new(),
            dialog_text: Vec::new(),
            s3: Sprites::default(),
            s4: Sprites::default(),
            dialog_seq: layout.sequences[0].clone(),
            widths,
            help: None,
            frames: 0,
        }
    }

    fn text(bytes: Vec<u8>, size: f32, align: u8) -> Text {
        let mut t = Text::new(bytes, size);
        t.align = align;
        t.group_position = false;
        t.keep_rgb = false;
        t
    }

    fn row_text(&self, bytes: Vec<u8>, seq0: &Sequence, group: usize) -> Text {
        let mut t = Self::text(bytes, TEXT_SIZE, 0);
        t.offset = [seq0.groups[group].text_position_x as f32, 0.];
        t
    }

    fn clear(&mut self) {
        for id in self
            .top_rows
            .drain(..)
            .chain(self.tab.take())
            .chain(self.help.take())
        {
            self.s0.kill(id);
        }
        for c in self.cards.drain(..) {
            self.s0.kill(c.row);
            self.s1.kill(c.picture);
            self.s2.kill(c.icon);
            for t in c.texts {
                self.s2.kill(t);
            }
        }
        self.top_cursor = usize::MAX;
        self.list_start = usize::MAX;
    }

    fn clear_dialog(&mut self) {
        for id in self.dialog_rows.drain(..) {
            self.s0.kill(id);
        }
        for id in self.dialog.drain(..) {
            self.s3.kill(id);
        }
        for id in self.dialog_text.drain(..) {
            self.s4.kill(id);
        }
        self.dialog_key = None;
    }

    /// One frame: the player's presses in, the menu's events out.
    pub fn frame(
        &mut self,
        layout: &data::menu_layout::Layout,
        backend: &mut dyn Backend,
        pressed: &[Input],
    ) -> Vec<Event> {
        self.frames += 1;
        for &i in pressed {
            self.menu.input(i, backend);
        }
        self.menu.tick(backend);
        let events = self.menu.take_events();
        self.sync(layout);
        self.frame.update(layout, 1.);
        self.s0.update(&layout.sequences[0]);
        self.s1.update(&layout.sequences[1]);
        self.s2.update(&layout.sequences[2]);
        self.s3.update(&self.dialog_seq);
        self.s4.update(&layout.sequences[2]);
        events
    }

    /// Start the close animation (the exit scene) and finish when done.
    pub fn close(&mut self, layout: &data::menu_layout::Layout) {
        if !self.closing {
            self.closing = true;
            self.frame.set_scene(SCENE[2], None);
            let seq0 = &layout.sequences[0];
            for &id in &self.top_rows {
                self.s0.close(seq0, id, TOP_ROW[1]);
            }
            for c in &self.cards {
                self.s0.close(seq0, c.row, CARD[1]);
            }
            if let Some(t) = self.tab {
                self.s0.close(seq0, t, TAB[1]);
            }
        }
    }

    pub fn is_finished(&self) -> bool {
        self.closing && self.frame.dead
    }

    /// Rebuilds or retargets the sprites to match the menu's view.
    fn sync(&mut self, layout: &data::menu_layout::Layout) {
        if self.closing {
            return;
        }
        let seq0 = &layout.sequences[0];
        let view = self.menu.view();
        let want = match (&view.top, view.list) {
            (Some(_), _) => Shown::Top,
            (_, Some(p)) => Shown::List(p),
            _ => self.shown,
        };
        let dialog_key = view.dialog.map(|d| match d {
            Dialog::Ask { message, .. } => (message, 0u8),
            Dialog::Info { message } => (message, 1),
            Dialog::Notice { message } => (message, 2),
        });
        let dialog = view.dialog;
        let top = view.top.clone();
        let list = view.list;
        drop(view);
        if want != self.shown {
            self.clear();
            self.shown = want;
            match want {
                Shown::Top => self.build_top(seq0, top.as_ref().map_or(0, |t| t.1)),
                Shown::List(p) => self.build_list(layout, p),
                Shown::None => {}
            }
        }
        match self.shown {
            Shown::Top => {
                if let Some((rows, cursor)) = top {
                    self.top_rows_sync(seq0, rows.len(), cursor);
                    self.set_help(seq0, rows.get(cursor).map(|r| r.1));
                }
            }
            Shown::List(p) => {
                let _ = list;
                self.list_sync(layout, p);
                let help = self.menu.help();
                self.set_help(seq0, help);
            }
            Shown::None => {}
        }
        if dialog_key != self.dialog_key {
            self.clear_dialog();
            self.dialog_key = dialog_key;
            if let Some(d) = dialog {
                self.build_dialog(seq0, &layout.sequences[2], d);
            }
        }
        if let Some(Dialog::Ask { cursor, .. }) = dialog {
            self.ask_cursor(seq0, cursor);
        }
    }

    fn set_help(&mut self, seq0: &Sequence, id: Option<u32>) {
        let bytes = id.map(|i| self.texts.message(i)).unwrap_or_default();
        match self.help {
            Some(h) => {
                self.s0.get_mut(h).text = Some(Self::text(bytes, TEXT_SIZE, 0));
            }
            None => {
                let h = self.s0.create(
                    seq0,
                    PRIO_TAB,
                    0,
                    22,
                    None,
                    Some(Self::text(bytes, TEXT_SIZE, 0)),
                );
                self.s0.get_mut(h).hidden = true;
                let mut t = Self::text(Vec::new(), TEXT_SIZE, 0);
                t.group_position = true;
                self.s0.get_mut(h).text = Some(t);
                self.help = Some(h);
            }
        }
    }

    fn build_top(&mut self, seq0: &Sequence, cursor: usize) {
        let label = match self.menu.kind() {
            save::menu::Kind::Title => message::LOAD,
            save::menu::Kind::SavePoint { .. } => message::SAVE,
        };
        let tab = self.s0.create(
            seq0,
            PRIO_TAB,
            0,
            TAB[0],
            Some(TAB[2]),
            Some(Self::text(self.texts.message(label), TEXT_SIZE, 0)),
        );
        self.tab = Some(tab);
        let rows = self.menu.top_rows();
        for (i, (label, _)) in rows.iter().enumerate() {
            let text = self.row_text(self.texts.message(*label), seq0, TOP_ROW[0]);
            let queued = if i == cursor { TOP_ROW[3] } else { TOP_ROW[2] };
            let id = self
                .s0
                .create(seq0, PRIO_ROWS, 0, TOP_ROW[0], Some(queued), Some(text));
            self.s0.get_mut(id).offset = [0., TOP_PITCH * i as f32];
            self.top_rows.push(id);
        }
        self.top_cursor = cursor;
    }

    fn top_rows_sync(&mut self, seq0: &Sequence, rows: usize, cursor: usize) {
        if cursor == self.top_cursor || rows != self.top_rows.len() {
            return;
        }
        for (i, &id) in self.top_rows.clone().iter().enumerate() {
            let group = if i == cursor { TOP_ROW[3] } else { TOP_ROW[2] };
            self.s0.set(seq0, id, group, None);
        }
        self.top_cursor = cursor;
    }

    fn build_list(&mut self, layout: &data::menu_layout::Layout, purpose: Purpose) {
        let seq0 = &layout.sequences[0];
        let label = match purpose {
            Purpose::Save => message::SAVE,
            Purpose::Load => message::LOAD,
        };
        let tab = self.s0.create(
            seq0,
            PRIO_TAB,
            0,
            TAB[0],
            Some(TAB[2]),
            Some(Self::text(self.texts.message(label), TEXT_SIZE, 0)),
        );
        self.tab = Some(tab);
        for i in 0..ROWS {
            let row = self
                .s0
                .create(seq0, PRIO_ROWS, 0, CARD[0], Some(CARD[2]), None);
            self.s0.get_mut(row).offset = [0., CARD_PITCH * i as f32];
            let picture = self
                .s1
                .create(&layout.sequences[1], PRIO_CARD_ART, 1, 0, None, None);
            let icon = self
                .s2
                .create(&layout.sequences[2], PRIO_CARD_ART, 2, 0, None, None);
            self.cards.push(Card {
                row,
                picture,
                icon,
                texts: Vec::new(),
            });
        }
        self.list_start = usize::MAX;
    }

    fn list_sync(&mut self, layout: &data::menu_layout::Layout, purpose: Purpose) {
        if self.cards.len() != ROWS {
            return;
        }
        let cursor = self.menu.cursor;
        let start = self.menu.window_start();
        let seq0 = &layout.sequences[0];
        // Rows idle or selected.
        for (i, c) in self.cards.iter().enumerate() {
            let group = if i == cursor.row { CARD[3] } else { CARD[2] };
            if self.s0.get(c.row).player.group != group
                && self.s0.get(c.row).queued.is_none()
                && self.s0.get(c.row).player.group != CARD[0]
            {
                self.s0.set(seq0, c.row, group, None);
            }
        }
        // Contents once per window position, slot and slot-list change
        // (a save or a delete rewrites a card).
        let key = start * 1000 + cursor.slot;
        let revision = self.menu.revision();
        if key == self.list_start && revision == self.list_revision {
            return;
        }
        self.list_start = key;
        self.list_revision = revision;
        let seq1 = &layout.sequences[1];
        let seq2 = &layout.sequences[2];
        for i in 0..ROWS {
            let slot = (start + i) % SLOT_COUNT;
            let base = [0., CARD_ORIGIN_Y + CARD_PITCH * i as f32];
            // Drop the old text sprites.
            for t in std::mem::take(&mut self.cards[i].texts) {
                self.s2.kill(t);
            }
            let (bg, icon, texts) = self.card_content(slot, purpose);
            let card = &self.cards[i];
            self.s1.set(seq1, card.picture, bg, None);
            self.s2.set(seq2, card.icon, icon, None);
            self.s1.get_mut(card.picture).base = [0., 0.];
            self.s1.get_mut(card.picture).offset = base;
            self.s2.get_mut(card.icon).offset = base;
            let mut ids = Vec::new();
            for (bytes, (x, y, align)) in texts {
                let mut t = Self::text(bytes, CARD_TEXT_SIZE, align);
                t.offset = [x, y];
                let id = self.s2.create(seq2, PRIO_CARD_TEXT, 2, 0, None, Some(t));
                self.s2.get_mut(id).offset = base;
                ids.push(id);
            }
            self.cards[i].texts = ids;
        }
    }

    /// FUN_140323250 for one slot: picture group, icon group and the text
    /// pieces with their slots.
    #[allow(clippy::type_complexity)]
    fn card_content(
        &self,
        slot: usize,
        purpose: Purpose,
    ) -> (usize, usize, Vec<(Vec<u8>, (f32, f32, u8))>) {
        let number = encode_number(&format!("{:02}", slot + 1));
        let mut out = vec![(number, (50., -8., 2))];
        let Some(card) = self.menu.slots().get(slot) else {
            return (0, 0, out);
        };
        match card {
            Slot::Valid { summary, .. } => {
                let s: &Summary = summary;
                let level = [
                    self.texts.message(message::LEVEL),
                    vec![0x01],
                    encode_number(&format!("{:2}", s.level)),
                ]
                .concat();
                out.push((level, CARD_TEXT[0]));
                let munny = [
                    encode_number(&format!("{:8}", s.munny)),
                    vec![0x01],
                    self.texts.message(message::MUNNY),
                ]
                .concat();
                out.push((munny, CARD_TEXT[1]));
                if let Some(Some(id)) = message::DIFFICULTY.get(s.difficulty as usize) {
                    out.push((self.texts.message(*id), CARD_TEXT[2]));
                }
                let place = self
                    .texts
                    .places
                    .get(&(s.world, s.area))
                    .cloned()
                    .unwrap_or_else(|| self.texts.unknown_place.clone());
                out.push((place, CARD_TEXT[3]));
                let (h, m) = s.hours_minutes();
                out.push((encode_number(&format!("{h:3}:{m:02}")), CARD_TEXT[4]));
                let icon = if s.world == 0xff {
                    0
                } else if s.background_flag {
                    9
                } else {
                    WORLD_ICON[(s.world as usize).min(18)]
                };
                let bg = BACKGROUND[(s.background as usize) % 8];
                let icon = if s.final_flag { FINAL_ICON } else { icon };
                (bg, icon, out)
            }
            other => {
                let id = match other {
                    Slot::Empty => match purpose {
                        Purpose::Save => message::SAVE_NEW_FILE,
                        Purpose::Load => message::NO_DATA,
                    },
                    _ => message::CORRUPT,
                };
                out.push((self.texts.message(id), CARD_TEXT[5]));
                (0, 0, out)
            }
        }
    }

    fn build_dialog(&mut self, seq0: &Sequence, seq2: &Sequence, dialog: Dialog) {
        let (message_id, ask) = match dialog {
            Dialog::Ask { message, .. } => (message, true),
            Dialog::Info { message } | Dialog::Notice { message } => (message, false),
        };
        if message_id != 0 {
            // The message window: sequence 0's groups 4 (open), 5 (idle)
            // and 6 (close) from the file module's window table (0x1405ba528,
            // FUN_140322dd0 → FUN_140300620). Its parts are a fixed box that
            // the group's own translation centres on (256, 192); the lines
            // are drawn from a text-only sprite.
            let bytes = self.texts.message(message_id);
            let (w, h) = crate::dialogue::measure(&bytes, &self.widths);
            let id = self.s3.create(
                &self.dialog_seq,
                PRIO_DIALOG,
                0,
                WINDOW[0],
                Some(WINDOW[1]),
                None,
            );
            self.dialog.push(id);
            // `measure` is in the font's reference units; the lines are drawn
            // at TEXT_SIZE.
            let k = TEXT_SIZE / FONT_REFERENCE;
            let mut t = Self::text(bytes, TEXT_SIZE, 0);
            t.anim_colour = false;
            t.offset = [
                DIALOG_AT[0] - w as f32 * k / 2.,
                DIALOG_AT[1] - h as f32 * k / 2.,
            ];
            let text = self.s4.create(seq2, PRIO_DIALOG + 1, 2, 0, None, Some(t));
            self.dialog_text.push(text);
        }
        match dialog {
            Dialog::Ask { cursor, .. } => {
                for (i, label) in [message::NO, message::YES].into_iter().enumerate() {
                    let queued = if i == cursor { CHOICE[3] } else { CHOICE[2] };
                    let text = self.row_text(self.texts.message(label), seq0, CHOICE[0]);
                    let id =
                        self.s0
                            .create(seq0, PRIO_DIALOG, 0, CHOICE[0], Some(queued), Some(text));
                    let pitch = seq0.groups[CHOICE[0]].ui_padding as f32;
                    self.s0.get_mut(id).offset = [pitch * i as f32, 0.];
                    self.dialog_rows.push(id);
                }
            }
            Dialog::Notice { .. } => {
                let text = self.row_text(self.texts.message(message::OK), seq0, CHOICE[0]);
                let id =
                    self.s0
                        .create(seq0, PRIO_DIALOG, 0, CHOICE[0], Some(CHOICE[3]), Some(text));
                self.dialog_rows.push(id);
            }
            Dialog::Info { .. } => {}
        }
        let _ = ask;
    }

    fn ask_cursor(&mut self, seq0: &Sequence, cursor: usize) {
        for (i, &id) in self.dialog_rows.clone().iter().enumerate() {
            let want = if i == cursor { CHOICE[3] } else { CHOICE[2] };
            let s = self.s0.get(id);
            if s.player.group != want && s.queued.is_none() && s.player.group != CHOICE[0] {
                self.s0.set(seq0, id, want, None);
            }
        }
    }

    /// This frame's art and text for the host to draw.
    pub fn draw(&self, menu: &Menu2ld, font: &crate::field_command_menu::Font) -> Draw {
        let layout = &menu.layout;
        let mut art = Vec::new();
        for b in menu_screen::draw(menu, &self.frame, [0, 0]) {
            art.push((b.image, b.quads));
        }
        let mut sheets = Vec::new();
        sheets.extend(self.s1.draw(&layout.sequences[1], font));
        sheets.extend(self.s0.draw(&layout.sequences[0], font));
        sheets.extend(self.s2.draw(&layout.sequences[2], font));
        sheets.extend(self.s3.draw(&self.dialog_seq, font));
        sheets.extend(self.s4.draw(&layout.sequences[2], font));
        Draw { art, sheets }
    }
}
