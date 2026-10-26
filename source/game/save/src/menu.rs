//! The file menu's rules, apart from how it is drawn: the save point's
//! three-row menu, the 99-slot file lists for saving and loading, and the
//! questions and messages between them. Flows follow the screens of the
//! exe's file module (`FUN_1402e78e0` for the save point, `FUN_1402e5460`
//! for the title's Load): the screen ids below are the exe's.
//!
//! The memory-card states of the original (no card, unformatted card, not
//! enough free blocks, failed reads) have no counterpart for a file on disk
//! except a failed write or a damaged container; those show the PC build's
//! own messages (sys.bar 0x8a8b, 0x8a8c, 0x8a8d, 0x8aae).

use crate::{
    data::SaveData,
    slots::{SLOT_COUNT, Slot},
};

/// sys.bar message ids (`& 0x7fff`) the menu shows.
pub mod message {
    pub const SAVE: u32 = 0x8a7e;
    pub const LOAD: u32 = 0x8a7f;
    pub const WORLD_MAP: u32 = 0x8a80;
    pub const TITLE_SCREEN: u32 = 0x8a81;
    /// "Jump to World Map?" and "Return to the title screen."
    pub const JUMP_QUESTION: u32 = 0x8a82;
    pub const RETURN_TO_TITLE_HELP: u32 = 0x8a88;
    /// Help for the Save row ("Save data to storage device.") and the
    /// World Map row.
    pub const SAVE_HELP: u32 = 0xb858;
    pub const WORLD_MAP_HELP: u32 = 0xb859;
    pub const EXIT_QUESTION: u32 = 0x8a8a;
    pub const FAILED_SAVE: u32 = 0x8a8b;
    pub const FAILED_CHECK: u32 = 0x8a8c;
    pub const FAILED_LOAD: u32 = 0x8a8d;
    pub const DELETING_PREVIOUS: u32 = 0x8a8f;
    pub const SAVING: u32 = 0x8a90;
    pub const LOADING: u32 = 0x8a91;
    pub const DELETING: u32 = 0x8a93;
    pub const FAILED_DELETE: u32 = 0x8a95;
    pub const CORRUPT: u32 = 0x8a9a;
    pub const SAVE_NEW_FILE: u32 = 0x8a9d;
    pub const NO_SPACE: u32 = 0x8a9e;
    pub const NO_DATA: u32 = 0x8a9f;
    pub const HELP_SAVE_LIST: u32 = 0x8aa1;
    pub const HELP_LOAD_LIST: u32 = 0x8aa2;
    pub const OVERWRITE_QUESTION: u32 = 0x8aa4;
    pub const DELETE_QUESTION: u32 = 0x8aa6;
    pub const FILE_SYSTEM_CORRUPT: u32 = 0x8aae;
    /// Row labels of the two-row question window (`0x9fd1`, `0x9fd0`) and
    /// of the one-row notice window.
    pub const NO: u32 = 0x9fd1;
    pub const YES: u32 = 0x9fd0;
    pub const OK: u32 = 0x9fcf;
    /// The labels the file cards use: "LV", "Munny", beginner / proud /
    /// critical mode.
    pub const LEVEL: u32 = 0xcd96;
    pub const MUNNY: u32 = 0xb7db;
    pub const DIFFICULTY: [Option<u32>; 4] = [Some(0x991e), None, Some(0x991f), Some(0xce32)];
}

/// Which menu is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A save point (`world_map` false) or world point (true): Save, World
    /// Map (world point only) and Title Screen.
    SavePoint { world_map: bool },
    /// The title's Load list.
    Title,
}

/// What the player pressed this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    Up,
    Down,
    /// Shoulder buttons: a page up or down in the list (4 rows).
    PageUp,
    PageDown,
    Decide,
    Back,
    Left,
    Right,
    /// The "Delete file" button (the glyph in message 0x8aa1).
    Delete,
}

/// se000 sound numbers the menu asks for (FUN_1402f6270, FUN_1402eb740).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sound {
    Move = 1,
    Decide = 2,
    Back = 3,
    Error = 4,
    Open = 0xb,
    Saved = 0xc,
    Loaded = 0xd,
}

/// What the menu tells the game.
#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    Sound(Sound),
    /// The state was written to `slot`.
    Saved {
        slot: usize,
    },
    /// The player chose `slot` to load; `data` is its verified image.
    Loaded {
        slot: usize,
        data: Box<SaveData>,
    },
    /// "Jump to World Map?" answered Yes (the game fades and jumps).
    JumpToWorldMap,
    /// "Title Screen" confirmed.
    ReturnToTitle,
    /// The menu was left without a choice.
    Closed,
}

/// The save store as the menu sees it.
pub trait Backend {
    fn slots(&self) -> Vec<Slot>;
    /// The verified image in `slot`.
    fn load(&mut self, slot: usize) -> Option<SaveData>;
    fn save(&mut self, slot: usize, data: &SaveData) -> bool;
    fn delete(&mut self, slot: usize) -> bool;
    /// The live game state as a save image (FUN_1402ed110).
    fn capture(&mut self) -> SaveData;
}

/// Frames a message window stays up while the operation behind it runs:
/// the window's opening animation plus the operation's few steps (the
/// original polls once per frame after the window is ready).
pub const WORK_FRAMES: u32 = 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    Save,
    Load,
}

/// A question with the No / Yes rows (No is row 0, the default).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ask {
    JumpToWorldMap,
    ReturnToTitle,
    Overwrite(usize),
    Delete(usize),
}

impl Ask {
    pub fn message(self) -> u32 {
        match self {
            Self::JumpToWorldMap => message::JUMP_QUESTION,
            Self::ReturnToTitle => message::EXIT_QUESTION,
            Self::Overwrite(_) => message::OVERWRITE_QUESTION,
            Self::Delete(_) => message::DELETE_QUESTION,
        }
    }
}

/// What a timed message does when its time is up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Work {
    Save(usize),
    Load(usize),
    Delete(usize),
    /// The fade before leaving: the event fires when it ends.
    Leave(Leave),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Leave {
    WorldMap,
    Title,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Screen {
    Top {
        cursor: usize,
    },
    List {
        purpose: Purpose,
    },
    Ask {
        ask: Ask,
        cursor: usize,
    },
    Work {
        text: u32,
        frames: u32,
        work: Work,
    },
    /// A one-row OK window over a screen.
    Notice {
        text: u32,
    },
    /// The menu has told the game to leave; nothing more happens.
    Done,
}

/// The list's cursor: the slot under it and the row (0..4) it sits on in
/// the four-row window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cursor {
    pub slot: usize,
    pub row: usize,
}

/// Rows the window shows at once.
pub const ROWS: usize = 4;

pub struct Menu {
    kind: Kind,
    screen: Screen,
    /// The screens under the current one (a dialog over a list over the top
    /// menu).
    stack: Vec<Screen>,
    slots: Vec<Slot>,
    /// Counts the slot list's rereads (after a save or a delete).
    revision: u32,
    pub cursor: Cursor,
    events: Vec<Event>,
    /// The slot last saved or loaded in this process (FUN_1402e3980).
    last_slot: Option<usize>,
}

impl Menu {
    pub fn new(kind: Kind, backend: &dyn Backend, last_slot: Option<usize>) -> Self {
        let slots = backend.slots();
        let screen = match kind {
            Kind::SavePoint { .. } => Screen::Top { cursor: 0 },
            Kind::Title => Screen::List {
                purpose: Purpose::Load,
            },
        };
        let mut menu = Self {
            kind,
            screen,
            stack: Vec::new(),
            cursor: Cursor { slot: 0, row: 0 },
            slots,
            revision: 0,
            events: vec![Event::Sound(Sound::Open)],
            last_slot,
        };
        menu.place_cursor();
        menu
    }

    /// FUN_1402e3880 / FUN_1402eda20: the last slot used, else the slot
    /// written most recently, else the first.
    fn place_cursor(&mut self) {
        let slot = self
            .last_slot
            .filter(|&s| s < SLOT_COUNT)
            .unwrap_or_else(|| {
                self.slots
                    .iter()
                    .enumerate()
                    .filter_map(|(i, s)| match s {
                        Slot::Valid { modified, .. } => Some((i, *modified)),
                        _ => None,
                    })
                    // The original keeps the later of equal times.
                    .fold(None::<(usize, i64)>, |best, (i, t)| match best {
                        Some((_, bt)) if t < bt => best,
                        _ => Some((i, t)),
                    })
                    .map_or(0, |(i, _)| i)
            });
        self.cursor = Cursor { slot, row: 0 };
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    pub fn slots(&self) -> &[Slot] {
        &self.slots
    }

    /// Changes whenever the slot list is read again, so a view knows its
    /// cards are stale.
    pub fn revision(&self) -> u32 {
        self.revision
    }

    pub fn last_slot(&self) -> Option<usize> {
        self.last_slot
    }

    /// The events since the last call.
    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    fn sound(&mut self, s: Sound) {
        self.events.push(Event::Sound(s));
    }

    /// Rows of the top menu: (label, help) message ids.
    pub fn top_rows(&self) -> Vec<(u32, u32)> {
        let mut rows = vec![(message::SAVE, message::SAVE_HELP)];
        if matches!(self.kind, Kind::SavePoint { world_map: true }) {
            rows.push((message::WORLD_MAP, message::WORLD_MAP_HELP));
        }
        rows.push((message::TITLE_SCREEN, message::RETURN_TO_TITLE_HELP));
        rows
    }

    /// What the screen shows now.
    pub fn view(&self) -> View<'_> {
        match &self.screen {
            Screen::Top { cursor } => View {
                top: Some((self.top_rows(), *cursor)),
                ..View::empty(self)
            },
            Screen::List { purpose } => View {
                list: Some(*purpose),
                ..View::empty(self)
            },
            Screen::Ask { ask, cursor } => {
                let (top, list) = self.behind_view();
                View {
                    top,
                    list,
                    dialog: Some(Dialog::Ask {
                        message: ask.message(),
                        cursor: *cursor,
                    }),
                    ..View::empty(self)
                }
            }
            Screen::Work { text, .. } => {
                let (top, list) = self.behind_view();
                View {
                    top,
                    list,
                    dialog: Some(Dialog::Info { message: *text }),
                    ..View::empty(self)
                }
            }
            Screen::Notice { text } => {
                let (top, list) = self.behind_view();
                View {
                    top,
                    list,
                    dialog: Some(Dialog::Notice { message: *text }),
                    ..View::empty(self)
                }
            }
            Screen::Done => View::empty(self),
        }
    }

    fn behind_view(&self) -> (Option<TopView>, Option<Purpose>) {
        match self.stack.last() {
            Some(Screen::Top { cursor }) => (Some((self.top_rows(), *cursor)), None),
            Some(Screen::List { purpose }) => (None, Some(*purpose)),
            _ => (None, None),
        }
    }

    /// The help line under the window for the current row.
    pub fn help(&self) -> Option<u32> {
        match &self.screen {
            Screen::Top { cursor } => self.top_rows().get(*cursor).map(|r| r.1),
            Screen::List { purpose } => Some(match purpose {
                Purpose::Save => message::HELP_SAVE_LIST,
                Purpose::Load => message::HELP_LOAD_LIST,
            }),
            _ => None,
        }
    }

    /// The text a list card shows for `slot` when it holds no usable save
    /// (FUN_140323250): None means the card draws the save's summary.
    pub fn card_message(&self, slot: usize, purpose: Purpose) -> Option<u32> {
        match self.slots.get(slot)? {
            Slot::Empty => Some(match purpose {
                Purpose::Save => message::SAVE_NEW_FILE,
                Purpose::Load => message::NO_DATA,
            }),
            Slot::Corrupt => Some(message::CORRUPT),
            Slot::Valid { .. } => None,
        }
    }

    fn open(&mut self, screen: Screen) {
        let below = std::mem::replace(&mut self.screen, screen);
        self.stack.push(below);
    }

    fn close_dialog(&mut self) {
        if let Some(below) = self.stack.pop() {
            self.screen = below;
        }
    }

    fn step_cursor(&mut self, delta: isize) {
        let n = SLOT_COUNT as isize;
        let slot = (self.cursor.slot as isize + delta).rem_euclid(n) as usize;
        let row = (self.cursor.row as isize + delta).clamp(0, ROWS as isize - 1) as usize;
        self.cursor = Cursor { slot, row };
    }

    /// The first slot shown in the four-row window.
    pub fn window_start(&self) -> usize {
        (self.cursor.slot + SLOT_COUNT - self.cursor.row) % SLOT_COUNT
    }

    /// One press. Timed screens advance with [`Menu::tick`].
    pub fn input(&mut self, input: Input, backend: &mut dyn Backend) {
        match self.screen.clone() {
            Screen::Top { cursor } => self.top_input(input, cursor),
            Screen::List { purpose } => self.list_input(input, purpose, backend),
            Screen::Ask { ask, cursor } => self.ask_input(input, ask, cursor),
            Screen::Notice { .. } => {
                if matches!(input, Input::Decide | Input::Back) {
                    self.sound(Sound::Decide);
                    self.close_dialog();
                }
            }
            Screen::Work { .. } | Screen::Done => {}
        }
    }

    fn top_input(&mut self, input: Input, cursor: usize) {
        let rows = self.top_rows();
        match input {
            Input::Left | Input::Right | Input::PageUp | Input::PageDown | Input::Delete => {}
            Input::Up | Input::Down => {
                let n = rows.len();
                let next = if input == Input::Up {
                    (cursor + n - 1) % n
                } else {
                    (cursor + 1) % n
                };
                if next != cursor {
                    self.sound(Sound::Move);
                }
                self.screen = Screen::Top { cursor: next };
            }
            Input::Back => {
                self.sound(Sound::Back);
                self.events.push(Event::Closed);
            }
            Input::Decide => {
                self.sound(Sound::Decide);
                let label = rows[cursor].0;
                match label {
                    message::SAVE => {
                        self.open(Screen::List {
                            purpose: Purpose::Save,
                        });
                        self.place_cursor();
                    }
                    message::WORLD_MAP => {
                        self.open(Screen::Ask {
                            ask: Ask::JumpToWorldMap,
                            cursor: 0,
                        });
                    }
                    _ => {
                        self.open(Screen::Ask {
                            ask: Ask::ReturnToTitle,
                            cursor: 0,
                        });
                    }
                }
            }
        }
    }

    fn list_input(&mut self, input: Input, purpose: Purpose, backend: &mut dyn Backend) {
        match input {
            Input::Up => {
                self.step_cursor(-1);
                self.sound(Sound::Move);
            }
            Input::Down => {
                self.step_cursor(1);
                self.sound(Sound::Move);
            }
            Input::PageUp => {
                self.step_cursor(-(ROWS as isize));
                self.sound(Sound::Move);
            }
            Input::PageDown => {
                self.step_cursor(ROWS as isize);
                self.sound(Sound::Move);
            }
            Input::Back => {
                self.sound(Sound::Back);
                match self.kind {
                    Kind::SavePoint { .. } => {
                        if self.stack.is_empty() {
                            self.screen = Screen::Top { cursor: 0 };
                        } else {
                            self.close_dialog();
                        }
                    }
                    Kind::Title => self.events.push(Event::Closed),
                }
            }
            Input::Left | Input::Right => {}
            Input::Delete => {
                let slot = self.cursor.slot;
                if self.slots[slot].is_valid() {
                    self.sound(Sound::Decide);
                    self.open(Screen::Ask {
                        ask: Ask::Delete(slot),
                        cursor: 0,
                    });
                }
            }
            Input::Decide => self.list_decide(purpose, backend),
        }
    }

    fn list_decide(&mut self, purpose: Purpose, _backend: &mut dyn Backend) {
        let slot = self.cursor.slot;
        match purpose {
            Purpose::Load => {
                if self.slots[slot].is_valid() {
                    self.sound(Sound::Decide);
                    self.open(Screen::Work {
                        text: message::LOADING,
                        frames: WORK_FRAMES,
                        work: Work::Load(slot),
                    });
                } else {
                    self.sound(Sound::Error);
                }
            }
            Purpose::Save => {
                self.sound(Sound::Decide);
                if self.slots[slot].is_valid() {
                    self.open(Screen::Ask {
                        ask: Ask::Overwrite(slot),
                        cursor: 0,
                    });
                } else {
                    // An empty slot, or a damaged file, is written at once.
                    self.open(Screen::Work {
                        text: message::SAVING,
                        frames: WORK_FRAMES,
                        work: Work::Save(slot),
                    });
                }
            }
        }
    }

    fn ask_input(&mut self, input: Input, ask: Ask, cursor: usize) {
        match input {
            // The rows sit side by side: Yes on the left, No on the right.
            Input::Left | Input::Right => {
                let next = usize::from(input == Input::Left);
                if next != cursor {
                    self.sound(Sound::Move);
                    self.screen = Screen::Ask { ask, cursor: next };
                }
            }
            Input::Back => {
                self.sound(Sound::Back);
                self.close_dialog();
            }
            Input::Decide if cursor == 0 => {
                self.sound(Sound::Back);
                self.close_dialog();
            }
            Input::Decide => {
                self.sound(Sound::Decide);
                self.confirm(ask);
            }
            Input::Up | Input::Down | Input::PageUp | Input::PageDown | Input::Delete => {}
        }
    }

    fn confirm(&mut self, ask: Ask) {
        let (text, work) = match ask {
            Ask::JumpToWorldMap => (0, Work::Leave(Leave::WorldMap)),
            Ask::ReturnToTitle => (0, Work::Leave(Leave::Title)),
            Ask::Overwrite(slot) => (message::DELETING_PREVIOUS, Work::Save(slot)),
            Ask::Delete(slot) => (message::DELETING, Work::Delete(slot)),
        };
        let frames = if text == 0 { 2 } else { WORK_FRAMES };
        self.screen = Screen::Work { text, frames, work };
    }

    /// One frame of timed messages.
    pub fn tick(&mut self, backend: &mut dyn Backend) {
        let Screen::Work { text, frames, work } = self.screen.clone() else {
            return;
        };
        if frames > 1 {
            self.screen = Screen::Work {
                text,
                frames: frames - 1,
                work,
            };
            return;
        }
        match work {
            Work::Save(slot) => self.finish_save(text, slot, backend),
            Work::Load(slot) => match backend.load(slot) {
                Some(data) => {
                    self.last_slot = Some(slot);
                    self.sound(Sound::Loaded);
                    self.events.push(Event::Loaded {
                        slot,
                        data: Box::new(data),
                    });
                    self.close_dialog();
                }
                None => self.fail(message::FAILED_LOAD),
            },
            Work::Delete(slot) => {
                if backend.delete(slot) {
                    self.slots = backend.slots();
                    self.revision += 1;
                    self.close_dialog();
                } else {
                    self.fail(message::FAILED_DELETE);
                }
            }
            Work::Leave(Leave::WorldMap) => {
                self.events.push(Event::JumpToWorldMap);
                self.screen = Screen::Done;
            }
            Work::Leave(Leave::Title) => {
                self.events.push(Event::ReturnToTitle);
                self.screen = Screen::Done;
            }
        }
    }

    /// The "Deleting previous data" message of an overwrite is followed by
    /// the "Saving data" one before the write (screens 9 -> 0xe).
    fn finish_save(&mut self, shown: u32, slot: usize, backend: &mut dyn Backend) {
        if shown == message::DELETING_PREVIOUS {
            self.screen = Screen::Work {
                text: message::SAVING,
                frames: WORK_FRAMES,
                work: Work::Save(slot),
            };
            return;
        }
        let data = backend.capture();
        if backend.save(slot, &data) {
            self.slots = backend.slots();
            self.revision += 1;
            self.last_slot = Some(slot);
            self.sound(Sound::Saved);
            self.events.push(Event::Saved { slot });
            self.close_dialog();
        } else {
            self.fail(message::FAILED_SAVE);
        }
    }

    fn fail(&mut self, text: u32) {
        self.sound(Sound::Error);
        self.screen = Screen::Notice { text };
    }
}

/// The top menu: its rows (label, help) and the cursor.
pub type TopView = (Vec<(u32, u32)>, usize);

/// A modal window over the list or the top menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dialog {
    /// A question with No (row 0) and Yes (row 1).
    Ask { message: u32, cursor: usize },
    /// A message while an operation runs.
    Info { message: u32 },
    /// An error with an OK row.
    Notice { message: u32 },
}

/// The screen as a description for the drawing code.
pub struct View<'a> {
    pub menu: &'a Menu,
    /// Top menu rows with the cursor.
    pub top: Option<TopView>,
    pub list: Option<Purpose>,
    pub dialog: Option<Dialog>,
}

impl<'a> View<'a> {
    fn empty(menu: &'a Menu) -> Self {
        Self {
            menu,
            top: None,
            list: None,
            dialog: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slots::Summary;

    struct Fake {
        slots: Vec<Slot>,
        saved: Vec<usize>,
        fail_save: bool,
    }

    fn valid(level: u8, modified: i64) -> Slot {
        Slot::Valid {
            summary: Summary {
                world: 2,
                area: 8,
                frames: 0,
                munny: 0,
                difficulty: 1,
                level,
                background: 0,
                background_flag: false,
                final_flag: false,
            },
            modified,
        }
    }

    impl Backend for Fake {
        fn slots(&self) -> Vec<Slot> {
            self.slots.clone()
        }
        fn load(&mut self, slot: usize) -> Option<SaveData> {
            self.slots[slot].is_valid().then(SaveData::blank)
        }
        fn save(&mut self, slot: usize, _: &SaveData) -> bool {
            if self.fail_save {
                return false;
            }
            self.slots[slot] = valid(1, 100);
            self.saved.push(slot);
            true
        }
        fn delete(&mut self, slot: usize) -> bool {
            self.slots[slot] = Slot::Empty;
            true
        }
        fn capture(&mut self) -> SaveData {
            SaveData::blank()
        }
    }

    fn fake() -> Fake {
        let mut slots = vec![Slot::Empty; SLOT_COUNT];
        slots[5] = valid(10, 50);
        slots[7] = valid(20, 90);
        slots[9] = Slot::Corrupt;
        Fake {
            slots,
            saved: Vec::new(),
            fail_save: false,
        }
    }

    fn run(menu: &mut Menu, b: &mut Fake, frames: u32) {
        for _ in 0..frames {
            menu.tick(b);
        }
    }

    fn events(menu: &mut Menu) -> Vec<Event> {
        menu.take_events()
            .into_iter()
            .filter(|e| !matches!(e, Event::Sound(_)))
            .collect()
    }

    #[test]
    fn cursor_starts_on_the_latest_save_unless_one_was_just_used() {
        let b = fake();
        let m = Menu::new(Kind::Title, &b, None);
        assert_eq!(m.cursor.slot, 7);
        let m = Menu::new(Kind::Title, &b, Some(3));
        assert_eq!(m.cursor.slot, 3);
    }

    #[test]
    fn save_point_has_two_rows_and_the_world_point_three() {
        let b = fake();
        let sp = Menu::new(Kind::SavePoint { world_map: false }, &b, None);
        assert_eq!(
            sp.top_rows().iter().map(|r| r.0).collect::<Vec<_>>(),
            [message::SAVE, message::TITLE_SCREEN]
        );
        let wp = Menu::new(Kind::SavePoint { world_map: true }, &b, None);
        assert_eq!(wp.top_rows().len(), 3);
    }

    #[test]
    fn saving_into_an_empty_slot_writes_then_returns_to_the_list() {
        let mut b = fake();
        let mut m = Menu::new(Kind::SavePoint { world_map: false }, &b, None);
        m.input(Input::Decide, &mut b); // Save
        assert!(m.view().list == Some(Purpose::Save));
        m.input(Input::Up, &mut b); // slot 6 (empty)
        assert_eq!(m.cursor.slot, 6);
        m.input(Input::Decide, &mut b);
        assert!(matches!(
            m.view().dialog,
            Some(Dialog::Info {
                message: message::SAVING
            })
        ));
        run(&mut m, &mut b, WORK_FRAMES);
        assert_eq!(events(&mut m), [Event::Saved { slot: 6 }]);
        assert_eq!(b.saved, [6]);
        assert!(m.view().dialog.is_none() && m.view().list.is_some());
        assert!(m.slots()[6].is_valid());
        // The view learns that its cards changed.
        assert_eq!(m.revision(), 1);
    }

    #[test]
    fn overwriting_asks_first_and_no_leaves_the_file_alone() {
        let mut b = fake();
        let mut m = Menu::new(Kind::SavePoint { world_map: false }, &b, None);
        m.input(Input::Decide, &mut b);
        assert_eq!(m.cursor.slot, 7);
        m.input(Input::Decide, &mut b);
        assert_eq!(
            m.view().dialog,
            Some(Dialog::Ask {
                message: message::OVERWRITE_QUESTION,
                cursor: 0
            })
        );
        m.input(Input::Decide, &mut b); // No (default)
        assert!(m.view().dialog.is_none());
        assert!(b.saved.is_empty());
        m.input(Input::Decide, &mut b);
        m.input(Input::Left, &mut b);
        m.input(Input::Decide, &mut b); // Yes
        run(&mut m, &mut b, WORK_FRAMES * 2);
        assert_eq!(events(&mut m), [Event::Saved { slot: 7 }]);
    }

    #[test]
    fn a_failed_write_shows_a_notice() {
        let mut b = fake();
        b.fail_save = true;
        let mut m = Menu::new(Kind::SavePoint { world_map: false }, &b, None);
        m.input(Input::Decide, &mut b);
        m.input(Input::Up, &mut b);
        m.input(Input::Decide, &mut b);
        run(&mut m, &mut b, WORK_FRAMES);
        assert_eq!(
            m.view().dialog,
            Some(Dialog::Notice {
                message: message::FAILED_SAVE
            })
        );
        m.input(Input::Decide, &mut b);
        assert!(m.view().dialog.is_none());
    }

    #[test]
    fn title_load_loads_a_valid_slot_and_refuses_the_others() {
        let mut b = fake();
        let mut m = Menu::new(Kind::Title, &b, None);
        assert_eq!(m.cursor.slot, 7);
        m.input(Input::Up, &mut b);
        m.input(Input::Decide, &mut b); // slot 6: empty
        assert!(m.view().dialog.is_none());
        m.input(Input::Down, &mut b);
        m.input(Input::Decide, &mut b);
        run(&mut m, &mut b, WORK_FRAMES);
        let ev = events(&mut m);
        assert!(matches!(ev.as_slice(), [Event::Loaded { slot: 7, .. }]));
        assert_eq!(m.last_slot(), Some(7));
    }

    #[test]
    fn delete_asks_then_removes() {
        let mut b = fake();
        let mut m = Menu::new(Kind::Title, &b, None);
        m.input(Input::Delete, &mut b);
        assert!(matches!(
            m.view().dialog,
            Some(Dialog::Ask {
                message: message::DELETE_QUESTION,
                ..
            })
        ));
        m.input(Input::Left, &mut b);
        m.input(Input::Decide, &mut b);
        run(&mut m, &mut b, WORK_FRAMES);
        assert_eq!(m.slots()[7], Slot::Empty);
        // Delete on an empty slot does nothing.
        m.input(Input::Delete, &mut b);
        assert!(m.view().dialog.is_none());
    }

    #[test]
    fn world_map_and_title_rows_leave_after_confirming() {
        let mut b = fake();
        let mut m = Menu::new(Kind::SavePoint { world_map: true }, &b, None);
        m.input(Input::Down, &mut b);
        m.input(Input::Decide, &mut b); // World Map
        m.input(Input::Left, &mut b);
        m.input(Input::Decide, &mut b);
        run(&mut m, &mut b, 3);
        assert_eq!(events(&mut m), [Event::JumpToWorldMap]);
        let mut m = Menu::new(Kind::SavePoint { world_map: true }, &b, None);
        m.input(Input::Down, &mut b);
        m.input(Input::Down, &mut b);
        m.input(Input::Decide, &mut b); // Title Screen
        m.input(Input::Back, &mut b); // No
        assert!(m.view().top.is_some());
        m.input(Input::Decide, &mut b);
        m.input(Input::Left, &mut b);
        m.input(Input::Decide, &mut b);
        run(&mut m, &mut b, 3);
        assert_eq!(events(&mut m), [Event::ReturnToTitle]);
    }

    #[test]
    fn back_closes_the_save_point_and_returns_from_the_list() {
        let mut b = fake();
        let mut m = Menu::new(Kind::SavePoint { world_map: false }, &b, None);
        m.input(Input::Decide, &mut b);
        m.input(Input::Back, &mut b);
        assert!(m.view().top.is_some());
        m.input(Input::Back, &mut b);
        assert_eq!(events(&mut m), [Event::Closed]);
    }

    #[test]
    fn list_cursor_wraps_and_windows_four_rows() {
        let b = fake();
        let mut m = Menu::new(Kind::Title, &b, Some(0));
        let mut bb = fake();
        m.input(Input::Up, &mut bb);
        assert_eq!(m.cursor.slot, SLOT_COUNT - 1);
        for _ in 0..3 {
            m.input(Input::Down, &mut bb);
        }
        assert_eq!((m.cursor.slot, m.cursor.row), (2, 3));
        assert_eq!(m.window_start(), SLOT_COUNT - 1);
    }
}
